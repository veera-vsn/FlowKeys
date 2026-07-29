use std::{sync::Mutex, thread, time::Duration};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::support::{generate_id, load_json, now_millis, save_json};

const CONFIG_FILE: &str = "clipboard_history.json";
const MAX_ENTRIES: usize = 500;
const POLL_INTERVAL: Duration = Duration::from_millis(600);
const UPDATED_EVENT: &str = "clipboard://updated";

#[derive(Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: String,
    pub text: String,
    pub copied_at: u64,
}

pub struct ClipboardState {
    /// Newest first.
    entries: Mutex<Vec<ClipboardEntry>>,
}

/// Loads persisted history, makes `ClipboardState` available to commands, and
/// starts a background thread that polls the system clipboard for changes.
/// Must run after the clipboard-manager plugin has been added to the app.
pub fn init(app: &AppHandle) -> Result<(), String> {
    let entries: Vec<ClipboardEntry> = load_json(app, CONFIG_FILE);
    app.manage(ClipboardState {
        entries: Mutex::new(entries),
    });

    let app_handle = app.clone();
    thread::spawn(move || watch_clipboard(app_handle));

    Ok(())
}

/// Runs on a dedicated background thread: `arboard`'s clipboard reads must not
/// happen on the main/UI thread, and Tauri has no native "clipboard changed"
/// event, so we poll.
fn watch_clipboard(app: AppHandle) {
    loop {
        thread::sleep(POLL_INTERVAL);
        let Ok(text) = app.clipboard().read_text() else {
            continue;
        };
        if text.trim().is_empty() {
            continue;
        }
        record_entry(&app, text);
    }
}

fn record_entry(app: &AppHandle, text: String) {
    let Some(state) = app.try_state::<ClipboardState>() else {
        return;
    };
    let mut entries = state.entries.lock().unwrap();
    if entries.first().map(|e| e.text.as_str()) == Some(text.as_str()) {
        return;
    }

    let entry = ClipboardEntry {
        id: generate_id("cb"),
        text,
        copied_at: now_millis(),
    };
    entries.insert(0, entry.clone());
    if entries.len() > MAX_ENTRIES {
        entries.truncate(MAX_ENTRIES);
    }
    let _ = save_json(app, CONFIG_FILE, &*entries);
    drop(entries);

    let _ = app.emit(UPDATED_EVENT, entry);
}

#[tauri::command]
pub fn list_clipboard_history(
    state: State<'_, ClipboardState>,
    query: Option<String>,
) -> Vec<ClipboardEntry> {
    let entries = state.entries.lock().unwrap();
    match query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        None => entries.clone(),
        Some(query) => {
            let query = query.to_lowercase();
            entries
                .iter()
                .filter(|e| e.text.to_lowercase().contains(&query))
                .cloned()
                .collect()
        }
    }
}

/// Writes an entry's text back to the system clipboard and bumps it to the
/// top of the history, mirroring how re-copying an item "touches" it.
#[tauri::command]
pub fn copy_clipboard_entry(app: AppHandle, state: State<'_, ClipboardState>, id: String) -> Result<(), String> {
    let mut entries = state.entries.lock().unwrap();
    let index = entries.iter().position(|e| e.id == id).ok_or("Entry not found.")?;
    let mut entry = entries.remove(index);
    entry.copied_at = now_millis();

    app.clipboard().write_text(entry.text.clone()).map_err(|e| e.to_string())?;

    entries.insert(0, entry);
    save_json(&app, CONFIG_FILE, &*entries)
}

#[tauri::command]
pub fn remove_clipboard_entry(app: AppHandle, state: State<'_, ClipboardState>, id: String) -> Result<(), String> {
    let mut entries = state.entries.lock().unwrap();
    let index = entries.iter().position(|e| e.id == id).ok_or("Entry not found.")?;
    entries.remove(index);
    save_json(&app, CONFIG_FILE, &*entries)
}

#[tauri::command]
pub fn clear_clipboard_history(app: AppHandle, state: State<'_, ClipboardState>) -> Result<(), String> {
    let mut entries = state.entries.lock().unwrap();
    entries.clear();
    save_json(&app, CONFIG_FILE, &*entries)
}
