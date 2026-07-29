use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
};

use enigo::{Direction, Enigo, Key as EnigoKey, Keyboard, Settings};
use rdev::{EventType, Key as RdevKey};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::clipboard;
use crate::support::{generate_id, load_json, now_millis, save_json};

const CONFIG_FILE: &str = "snippets.json";
const EXPANDED_EVENT: &str = "snippets://expanded";
// Comfortably longer than any reasonable trigger; keeps the buffer from
// growing unbounded while someone types a long sentence with no match.
const MAX_BUFFER_CHARS: usize = 100;
/// The keyboard hook sees a keystroke *before* the focused app does, so the
/// last characters of the trigger may still be in flight when we match. Wait
/// for them to land before backspacing, or we delete the wrong characters.
const PRE_EXPAND_DELAY: Duration = Duration::from_millis(90);
/// Spacing between individual synthetic backspaces. Firing them in an
/// unpaced burst outruns the target app's input queue and some get dropped.
const KEY_DELAY: Duration = Duration::from_millis(10);
/// Gap between the last backspace and the paste, so the deletions have all
/// been applied before the replacement arrives.
const MID_EXPAND_DELAY: Duration = Duration::from_millis(70);
/// Time for the target app to actually service Ctrl+V before we put the
/// user's own clipboard contents back.
const PASTE_SETTLE_DELAY: Duration = Duration::from_millis(150);
/// Our own synthetic keystrokes come back through the hook asynchronously.
/// Keep suppressing input until they've drained, or we re-scan our own output.
const POST_EXPAND_DELAY: Duration = Duration::from_millis(80);

#[derive(Clone, Serialize)]
pub struct ExpandedEvent {
    pub name: String,
    pub at: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub id: String,
    pub name: String,
    /// Text typed anywhere on the system that triggers the expansion, e.g. ";addr".
    pub trigger: String,
    pub content: String,
    pub enabled: bool,
}

pub struct SnippetState {
    snippets: Mutex<Vec<Snippet>>,
}

/// Loads persisted snippets, makes `SnippetState` available to commands, and
/// starts the background expansion engine (global keyboard hook).
pub fn init(app: &AppHandle) -> Result<(), String> {
    let snippets: Vec<Snippet> = load_json(app, CONFIG_FILE);
    app.manage(SnippetState {
        snippets: Mutex::new(snippets),
    });

    let app_handle = app.clone();
    thread::spawn(move || run_expansion_engine(app_handle));

    Ok(())
}

#[tauri::command]
pub fn list_snippets(state: State<'_, SnippetState>) -> Vec<Snippet> {
    state.snippets.lock().unwrap().clone()
}

#[tauri::command]
pub fn add_snippet(
    app: AppHandle,
    state: State<'_, SnippetState>,
    name: String,
    trigger: String,
    content: String,
) -> Result<Snippet, String> {
    let name = require_field(&name, "Name")?;
    let trigger = require_field(&trigger, "Trigger")?;
    let content = require_content(&content)?;

    let mut snippets = state.snippets.lock().unwrap();
    if let Some(existing) = snippets.iter().find(|s| s.trigger == trigger) {
        return Err(format!(
            "\"{trigger}\" is already used by \"{}\".",
            existing.name
        ));
    }

    let snippet = Snippet {
        id: generate_id("sn"),
        name,
        trigger,
        content,
        enabled: true,
    };
    snippets.push(snippet.clone());
    save_json(&app, CONFIG_FILE, &*snippets)?;
    Ok(snippet)
}

#[tauri::command]
pub fn update_snippet(
    app: AppHandle,
    state: State<'_, SnippetState>,
    id: String,
    name: String,
    trigger: String,
    content: String,
    enabled: bool,
) -> Result<Snippet, String> {
    let name = require_field(&name, "Name")?;
    let trigger = require_field(&trigger, "Trigger")?;
    let content = require_content(&content)?;

    let mut snippets = state.snippets.lock().unwrap();
    let index = snippets
        .iter()
        .position(|s| s.id == id)
        .ok_or("Snippet not found.")?;

    if let Some(conflict) = snippets
        .iter()
        .enumerate()
        .find(|(i, s)| *i != index && s.trigger == trigger)
        .map(|(_, s)| s.name.clone())
    {
        return Err(format!("\"{trigger}\" is already used by \"{conflict}\"."));
    }

    let updated = Snippet {
        id,
        name,
        trigger,
        content,
        enabled,
    };
    snippets[index] = updated.clone();
    save_json(&app, CONFIG_FILE, &*snippets)?;
    Ok(updated)
}

#[tauri::command]
pub fn remove_snippet(app: AppHandle, state: State<'_, SnippetState>, id: String) -> Result<(), String> {
    let mut snippets = state.snippets.lock().unwrap();
    let index = snippets
        .iter()
        .position(|s| s.id == id)
        .ok_or("Snippet not found.")?;
    snippets.remove(index);
    save_json(&app, CONFIG_FILE, &*snippets)
}

fn require_field(value: &str, label: &str) -> Result<String, String> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(format!("{label} is required."));
    }
    Ok(trimmed)
}

fn require_content(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        return Err("Expansion text is required.".into());
    }
    Ok(value.to_string())
}

/// Runs on a dedicated thread for the lifetime of the app: installs a global
/// low-level keyboard hook via `rdev` and tracks a rolling buffer of recently
/// typed characters. The instant the buffer's tail matches an enabled
/// snippet's trigger, it hands off to the injector thread, which backspaces
/// the trigger away and pastes the expansion in its place. Trigger keystrokes
/// are left to reach their target app normally and are corrected after the
/// fact, rather than intercepted — simpler and more robust than trying to
/// swallow input live.
///
/// Injection deliberately happens on a *separate* thread: on Windows the hook
/// callback runs inline on the system input chain, so synthesizing input from
/// inside it races with the very keystrokes that triggered it and can wedge
/// the focused app (including our own webview).
fn run_expansion_engine(app: AppHandle) {
    let injecting = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel::<Snippet>();

    {
        let app = app.clone();
        let injecting = Arc::clone(&injecting);
        thread::spawn(move || run_injector(app, rx, injecting));
    }

    let buffer = RefCell::new(String::new());

    let result = rdev::listen(move |event| {
        // Ignore everything while our own synthetic keystrokes are in flight.
        if injecting.load(Ordering::SeqCst) {
            return;
        }
        let EventType::KeyPress(key) = event.event_type else {
            return;
        };

        {
            let mut buf = buffer.borrow_mut();
            if key == RdevKey::Backspace {
                buf.pop();
            } else if let Some(text) = event.name.as_deref() {
                buf.extend(text.chars().filter(|c| !c.is_control()));
            }
            let overflow = buf.chars().count().saturating_sub(MAX_BUFFER_CHARS);
            if overflow > 0 {
                *buf = buf.chars().skip(overflow).collect();
            }
        }

        let Some(state) = app.try_state::<SnippetState>() else {
            return;
        };
        let matched = {
            let snippets = state.snippets.lock().unwrap();
            let buf = buffer.borrow();
            snippets
                .iter()
                .filter(|s| s.enabled && !s.trigger.is_empty() && buf.ends_with(s.trigger.as_str()))
                // Longest trigger wins, so ";addr" beats ";a" when both match.
                .max_by_key(|s| s.trigger.chars().count())
                .cloned()
        };

        let Some(snippet) = matched else {
            return;
        };

        // Claim the guard here rather than on the injector thread, so
        // keystrokes arriving in the handoff window can't slip through.
        injecting.store(true, Ordering::SeqCst);
        buffer.borrow_mut().clear();
        if tx.send(snippet).is_err() {
            injecting.store(false, Ordering::SeqCst);
        }
    });

    if let Err(err) = result {
        eprintln!("FlowKeys: couldn't start snippet expansion engine: {err:?}");
    }
}

/// Owns the input simulator and performs expansions off the hook thread.
/// Always clears the `injecting` guard it was handed, on every path.
fn run_injector(app: AppHandle, rx: mpsc::Receiver<Snippet>, injecting: Arc<AtomicBool>) {
    let mut enigo = match Enigo::new(&Settings::default()) {
        Ok(enigo) => enigo,
        Err(err) => {
            eprintln!("FlowKeys: couldn't initialize input simulator, snippet expansion disabled: {err}");
            injecting.store(false, Ordering::SeqCst);
            return;
        }
    };

    while let Ok(snippet) = rx.recv() {
        // Never expand into our own windows: the snippet editor is where
        // triggers get typed on purpose.
        if flowkeys_has_focus(&app) {
            injecting.store(false, Ordering::SeqCst);
            continue;
        }

        thread::sleep(PRE_EXPAND_DELAY);
        for _ in 0..snippet.trigger.chars().count() {
            let _ = enigo.key(EnigoKey::Backspace, Direction::Click);
            thread::sleep(KEY_DELAY);
        }
        thread::sleep(MID_EXPAND_DELAY);
        paste_text(&app, &mut enigo, &snippet.content);
        thread::sleep(POST_EXPAND_DELAY);

        injecting.store(false, Ordering::SeqCst);

        let _ = app.emit(
            EXPANDED_EVENT,
            ExpandedEvent {
                name: snippet.name,
                at: now_millis(),
            },
        );
    }
}

/// Inserts `text` by staging it on the clipboard and sending Ctrl+V, then
/// putting the user's own clipboard contents back.
///
/// Typing the expansion character-by-character via synthetic key events
/// proved unreliable in practice — characters arrived out of order, got
/// dropped, or mangled newlines, because each one is a separate event racing
/// through the input queue. A paste is a single atomic operation, so
/// multi-line and punctuation-heavy snippets come out intact.
fn paste_text(app: &AppHandle, enigo: &mut Enigo, text: &str) {
    // Keep this round-trip out of the user's clipboard history.
    clipboard::set_suppressed(app, true);
    let previous = app.clipboard().read_text().ok();

    if app.clipboard().write_text(text.to_string()).is_err() {
        clipboard::set_suppressed(app, false);
        return;
    }
    thread::sleep(KEY_DELAY);

    let _ = enigo.key(EnigoKey::Control, Direction::Press);
    let _ = enigo.key(EnigoKey::Unicode('v'), Direction::Click);
    let _ = enigo.key(EnigoKey::Control, Direction::Release);

    thread::sleep(PASTE_SETTLE_DELAY);

    // When nothing readable was there before (empty, or non-text content we
    // can't reproduce), leaving the snippet on the clipboard is a less
    // surprising outcome than silently clearing it.
    if let Some(previous) = previous {
        let _ = app.clipboard().write_text(previous);
    }
    clipboard::set_suppressed(app, false);
}

fn flowkeys_has_focus(app: &AppHandle) -> bool {
    app.webview_windows()
        .values()
        .any(|window| window.is_focused().unwrap_or(false))
}
