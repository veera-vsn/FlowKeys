use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::support::{load_json, save_json};

const CONFIG_FILE: &str = "settings.json";

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Settings {
    /// Copy text to the clipboard as soon as it's selected with the mouse,
    /// anywhere on the system. Off by default: it works by sending Ctrl+C
    /// after a drag, which in a terminal interrupts whatever is running.
    pub copy_on_selection: bool,
}

pub struct SettingsState {
    settings: Mutex<Settings>,
}

pub fn init(app: &AppHandle) -> Result<(), String> {
    let settings: Settings = load_json(app, CONFIG_FILE);
    app.manage(SettingsState {
        settings: Mutex::new(settings),
    });
    Ok(())
}

/// Cheap enough to call from the input hook on every mouse release.
pub fn copy_on_selection(app: &AppHandle) -> bool {
    app.try_state::<SettingsState>()
        .map(|state| state.settings.lock().unwrap().copy_on_selection)
        .unwrap_or(false)
}

#[tauri::command]
pub fn get_settings(state: State<'_, SettingsState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_copy_on_selection(
    app: AppHandle,
    state: State<'_, SettingsState>,
    enabled: bool,
) -> Result<Settings, String> {
    let mut settings = state.settings.lock().unwrap();
    settings.copy_on_selection = enabled;
    save_json(&app, CONFIG_FILE, &*settings)?;
    Ok(settings.clone())
}
