use std::{
    cell::{Cell, RefCell},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
};

use enigo::{Direction, Enigo, Key as EnigoKey, Keyboard, Settings};
use rdev::{Button, EventType, Key as RdevKey};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::support::{generate_id, load_json, now_millis, save_json};
use crate::{clipboard, settings, toast};

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
/// Minimum pointer travel, in pixels, before a click-and-drag counts as
/// selecting text. Keeps ordinary clicks and tiny jitters from firing Ctrl+C.
const DRAG_THRESHOLD: f64 = 6.0;
/// Time for the app to put the selection on the clipboard after Ctrl+C.
const COPY_SETTLE_DELAY: Duration = Duration::from_millis(120);

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

/// Work that must run off the input-hook thread, because it synthesizes
/// keystrokes and doing that inline on the hook wedges the focused app.
enum InjectorJob {
    /// Replace a typed trigger with its expansion.
    Expand(Snippet),
    /// Copy whatever the user just selected with the mouse.
    CopySelection,
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
    let (tx, rx) = mpsc::channel::<InjectorJob>();

    {
        let app = app.clone();
        let injecting = Arc::clone(&injecting);
        thread::spawn(move || run_injector(app, rx, injecting));
    }

    let buffer = RefCell::new(String::new());
    let pointer = Cell::new((0.0f64, 0.0f64));
    let drag_start = Cell::new(None::<(f64, f64)>);

    let result = rdev::listen(move |event| {
        // Ignore everything while our own synthetic keystrokes are in flight.
        if injecting.load(Ordering::SeqCst) {
            return;
        }

        match event.event_type {
            EventType::MouseMove { x, y } => {
                pointer.set((x, y));
                return;
            }
            // A click puts the caret somewhere we didn't track, so whatever we
            // buffered no longer describes the text in front of it.
            EventType::ButtonPress(button) => {
                buffer.borrow_mut().clear();
                if button == Button::Left {
                    drag_start.set(Some(pointer.get()));
                }
                return;
            }
            EventType::ButtonRelease(button) => {
                if button != Button::Left {
                    return;
                }
                let Some(start) = drag_start.take() else {
                    return;
                };
                if !settings::copy_on_selection(&app) {
                    return;
                }
                let (x, y) = pointer.get();
                if (x - start.0).hypot(y - start.1) < DRAG_THRESHOLD {
                    return;
                }
                injecting.store(true, Ordering::SeqCst);
                if tx.send(InjectorJob::CopySelection).is_err() {
                    injecting.store(false, Ordering::SeqCst);
                }
                return;
            }
            EventType::KeyPress(key) if resets_buffer(key) => {
                buffer.borrow_mut().clear();
                return;
            }
            EventType::KeyPress(key) => {
                push_key(&mut buffer.borrow_mut(), key, event.name.as_deref());
            }
            _ => return,
        }

        let Some(state) = app.try_state::<SnippetState>() else {
            return;
        };
        let matched = {
            let snippets = state.snippets.lock().unwrap();
            let buf = buffer.borrow();
            find_match(&snippets, &buf).cloned()
        };

        let Some(snippet) = matched else {
            return;
        };

        // Claim the guard here rather than on the injector thread, so
        // keystrokes arriving in the handoff window can't slip through.
        injecting.store(true, Ordering::SeqCst);
        buffer.borrow_mut().clear();
        if tx.send(InjectorJob::Expand(snippet)).is_err() {
            injecting.store(false, Ordering::SeqCst);
        }
    });

    if let Err(err) = result {
        eprintln!("FlowKeys: couldn't start snippet expansion engine: {err:?}");
    }
}

/// Keys that move the caret or commit the line. After any of them the buffer
/// no longer describes the text sitting immediately before the cursor, so
/// matching on it would backspace over characters the user never typed as
/// part of a trigger.
fn resets_buffer(key: RdevKey) -> bool {
    matches!(
        key,
        RdevKey::Return
            | RdevKey::KpReturn
            | RdevKey::Tab
            | RdevKey::Escape
            | RdevKey::UpArrow
            | RdevKey::DownArrow
            | RdevKey::LeftArrow
            | RdevKey::RightArrow
            | RdevKey::Home
            | RdevKey::End
            | RdevKey::PageUp
            | RdevKey::PageDown
            | RdevKey::Delete
            | RdevKey::Insert
    )
}

/// Folds one keystroke into the rolling buffer of recently typed text.
fn push_key(buffer: &mut String, key: RdevKey, name: Option<&str>) {
    if key == RdevKey::Backspace {
        buffer.pop();
        return;
    }
    if let Some(text) = name {
        buffer.extend(text.chars().filter(|c| !c.is_control()));
    }
    let overflow = buffer.chars().count().saturating_sub(MAX_BUFFER_CHARS);
    if overflow > 0 {
        *buffer = buffer.chars().skip(overflow).collect();
    }
}

/// The enabled snippet whose trigger ends `buffer`, preferring the longest so
/// ";addr" wins over ";a" when both would match.
fn find_match<'a>(snippets: &'a [Snippet], buffer: &str) -> Option<&'a Snippet> {
    snippets
        .iter()
        .filter(|s| s.enabled && !s.trigger.is_empty() && buffer.ends_with(s.trigger.as_str()))
        .max_by_key(|s| s.trigger.chars().count())
}

/// Owns the input simulator and performs expansions off the hook thread.
/// Always clears the `injecting` guard it was handed, on every path.
fn run_injector(app: AppHandle, rx: mpsc::Receiver<InjectorJob>, injecting: Arc<AtomicBool>) {
    let mut enigo = match Enigo::new(&Settings::default()) {
        Ok(enigo) => enigo,
        Err(err) => {
            eprintln!("FlowKeys: couldn't initialize input simulator, snippet expansion disabled: {err}");
            injecting.store(false, Ordering::SeqCst);
            return;
        }
    };

    while let Ok(job) = rx.recv() {
        // Never act on our own windows: the snippet editor is where triggers
        // get typed on purpose, and selections here are ordinary editing.
        if flowkeys_has_focus(&app) {
            injecting.store(false, Ordering::SeqCst);
            continue;
        }

        match job {
            InjectorJob::Expand(snippet) => expand(&app, &mut enigo, snippet, &injecting),
            InjectorJob::CopySelection => copy_selection(&app, &mut enigo, &injecting),
        }
    }
}

fn expand(app: &AppHandle, enigo: &mut Enigo, snippet: Snippet, injecting: &AtomicBool) {
    thread::sleep(PRE_EXPAND_DELAY);
    for _ in 0..snippet.trigger.chars().count() {
        let _ = enigo.key(EnigoKey::Backspace, Direction::Click);
        thread::sleep(KEY_DELAY);
    }
    thread::sleep(MID_EXPAND_DELAY);
    paste_text(app, enigo, &snippet.content);
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

/// Copies the current selection by sending Ctrl+C, then reports what landed
/// on the clipboard. Only reached when the user has opted in.
fn copy_selection(app: &AppHandle, enigo: &mut Enigo, injecting: &AtomicBool) {
    let before = app.clipboard().read_text().ok();

    let _ = enigo.key(EnigoKey::Control, Direction::Press);
    let _ = enigo.key(EnigoKey::Unicode('c'), Direction::Click);
    let _ = enigo.key(EnigoKey::Control, Direction::Release);

    thread::sleep(COPY_SETTLE_DELAY);
    injecting.store(false, Ordering::SeqCst);

    // A drag that selected nothing leaves the clipboard untouched; staying
    // silent there beats announcing a copy that never happened.
    let Ok(after) = app.clipboard().read_text() else {
        return;
    };
    if after.trim().is_empty() || Some(&after) == before.as_ref() {
        return;
    }
    toast::show(app, format!("Copied · {} characters", after.chars().count()));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn snippet(trigger: &str, enabled: bool) -> Snippet {
        Snippet {
            id: format!("id{trigger}"),
            name: format!("name{trigger}"),
            trigger: trigger.to_string(),
            content: format!("expansion for {trigger}"),
            enabled,
        }
    }

    fn type_text(buffer: &mut String, text: &str) {
        for ch in text.chars() {
            push_key(buffer, RdevKey::Unknown(0), Some(&ch.to_string()));
        }
    }

    #[test]
    fn matches_a_trigger_at_the_end_of_the_buffer() {
        let snippets = [snippet(";addr", true)];
        let matched = find_match(&snippets, "hello ;addr");
        assert_eq!(matched.map(|s| s.trigger.as_str()), Some(";addr"));
    }

    #[test]
    fn ignores_a_trigger_that_is_not_at_the_end() {
        let snippets = [snippet(";addr", true)];
        assert!(find_match(&snippets, ";addr and more typing").is_none());
    }

    #[test]
    fn prefers_the_longest_matching_trigger() {
        let snippets = [snippet(";a", true), snippet(";addr", true)];
        let matched = find_match(&snippets, "text ;addr");
        assert_eq!(matched.map(|s| s.trigger.as_str()), Some(";addr"));
    }

    #[test]
    fn skips_disabled_snippets() {
        let snippets = [snippet(";addr", false)];
        assert!(find_match(&snippets, "hello ;addr").is_none());
    }

    #[test]
    fn skips_empty_triggers_which_would_match_everything() {
        let snippets = [snippet("", true)];
        assert!(find_match(&snippets, "any text at all").is_none());
    }

    #[test]
    fn backspace_removes_the_last_buffered_character() {
        let mut buffer = String::new();
        type_text(&mut buffer, ";addr");
        push_key(&mut buffer, RdevKey::Backspace, None);
        assert_eq!(buffer, ";add");
    }

    #[test]
    fn backspace_on_an_empty_buffer_is_harmless() {
        let mut buffer = String::new();
        push_key(&mut buffer, RdevKey::Backspace, None);
        assert!(buffer.is_empty());
    }

    #[test]
    fn control_characters_never_enter_the_buffer() {
        let mut buffer = String::new();
        push_key(&mut buffer, RdevKey::Unknown(0), Some("\r\n"));
        assert!(buffer.is_empty());
    }

    #[test]
    fn buffer_is_trimmed_by_characters_not_bytes() {
        let mut buffer = String::new();
        // Multi-byte characters: trimming by byte offset would split one and
        // panic, or silently corrupt the buffer.
        type_text(&mut buffer, &"é".repeat(MAX_BUFFER_CHARS + 20));
        assert_eq!(buffer.chars().count(), MAX_BUFFER_CHARS);
    }

    #[test]
    fn a_trigger_still_matches_after_the_buffer_overflows() {
        let mut buffer = String::new();
        type_text(&mut buffer, &"x".repeat(MAX_BUFFER_CHARS * 2));
        type_text(&mut buffer, ";addr");
        let snippets = [snippet(";addr", true)];
        assert!(find_match(&snippets, &buffer).is_some());
    }

    #[test]
    fn caret_movement_keys_invalidate_the_buffer() {
        // Typing ";add", moving the caret, then typing "r" must NOT expand:
        // the buffered text no longer sits in front of the cursor, so
        // backspacing would eat characters the user never typed.
        for key in [
            RdevKey::LeftArrow,
            RdevKey::RightArrow,
            RdevKey::UpArrow,
            RdevKey::DownArrow,
            RdevKey::Home,
            RdevKey::End,
            RdevKey::Return,
            RdevKey::Tab,
            RdevKey::Delete,
        ] {
            assert!(resets_buffer(key), "{key:?} should invalidate the buffer");
        }
    }

    #[test]
    fn ordinary_typing_keys_do_not_invalidate_the_buffer() {
        for key in [RdevKey::KeyA, RdevKey::Space, RdevKey::SemiColon, RdevKey::Backspace] {
            assert!(!resets_buffer(key), "{key:?} should keep the buffer");
        }
    }

    #[test]
    fn required_fields_reject_blank_and_whitespace_only_input() {
        assert!(require_field("", "Name").is_err());
        assert!(require_field("   ", "Name").is_err());
        assert_eq!(require_field("  Home  ", "Name").unwrap(), "Home");
        assert!(require_content("   \n  ").is_err());
    }

    #[test]
    fn expansion_content_keeps_its_internal_whitespace_and_newlines() {
        // Multi-line snippets (addresses, signatures) must survive intact.
        let address = "85-2-7/11,\nVl puram,\nAndhra";
        assert_eq!(require_content(address).unwrap(), address);
    }
}
