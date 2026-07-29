use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const CONFIG_FILE: &str = "hotkeys.json";
const MAX_LOG_ENTRIES: usize = 50;
const TRIGGERED_EVENT: &str = "hotkeys://triggered";

#[derive(Clone, Serialize, Deserialize)]
pub struct HotkeyBinding {
    pub id: String,
    pub name: String,
    /// Canonical form produced by the shortcut parser, e.g. "control+shift+keyk".
    pub shortcut: String,
    pub enabled: bool,
}

#[derive(Clone, Serialize)]
pub struct TriggerLogEntry {
    pub id: String,
    pub name: String,
    pub shortcut: String,
    pub at: u64,
}

pub struct HotkeyState {
    bindings: Mutex<Vec<HotkeyBinding>>,
    log: Mutex<Vec<TriggerLogEntry>>,
}

/// Registers persisted hotkeys and makes `HotkeyState` available to commands.
/// Must run after the global-shortcut plugin has been added to the app.
pub fn init(app: &AppHandle) -> Result<(), String> {
    let mut bindings = load_bindings(app);
    for binding in bindings.iter_mut() {
        if !binding.enabled {
            continue;
        }
        if let Err(err) = register_with_handler(app, &binding.id, &binding.name, &binding.shortcut) {
            eprintln!(
                "FlowKeys: couldn't re-register hotkey \"{}\" ({}): {err}",
                binding.name, binding.shortcut
            );
            binding.enabled = false;
        }
    }
    save_bindings(app, &bindings)?;

    app.manage(HotkeyState {
        bindings: Mutex::new(bindings),
        log: Mutex::new(Vec::new()),
    });
    Ok(())
}

#[tauri::command]
pub fn list_hotkeys(state: State<'_, HotkeyState>) -> Vec<HotkeyBinding> {
    state.bindings.lock().unwrap().clone()
}

#[tauri::command]
pub fn recent_triggers(state: State<'_, HotkeyState>) -> Vec<TriggerLogEntry> {
    state.log.lock().unwrap().clone()
}

#[tauri::command]
pub fn add_hotkey(
    app: AppHandle,
    state: State<'_, HotkeyState>,
    name: String,
    shortcut: String,
) -> Result<HotkeyBinding, String> {
    let name = require_name(&name)?;
    let parsed = parse_and_validate(&shortcut)?;
    let canonical = parsed.into_string();

    let mut bindings = state.bindings.lock().unwrap();
    if let Some(existing) = bindings.iter().find(|b| shortcut_matches(b, parsed.id())) {
        return Err(format!(
            "\"{canonical}\" is already assigned to \"{}\".",
            existing.name
        ));
    }

    let id = generate_id();
    register_with_handler(&app, &id, &name, &canonical)?;

    let binding = HotkeyBinding {
        id,
        name,
        shortcut: canonical,
        enabled: true,
    };
    bindings.push(binding.clone());
    save_bindings(&app, &bindings)?;
    Ok(binding)
}

#[tauri::command]
pub fn update_hotkey(
    app: AppHandle,
    state: State<'_, HotkeyState>,
    id: String,
    name: String,
    shortcut: String,
    enabled: bool,
) -> Result<HotkeyBinding, String> {
    let name = require_name(&name)?;
    let parsed = parse_and_validate(&shortcut)?;
    let canonical = parsed.into_string();

    let mut bindings = state.bindings.lock().unwrap();
    let index = bindings
        .iter()
        .position(|b| b.id == id)
        .ok_or("Hotkey not found.")?;

    if let Some(conflict) = bindings
        .iter()
        .enumerate()
        .find(|(i, b)| *i != index && shortcut_matches(b, parsed.id()))
        .map(|(_, b)| b.name.clone())
    {
        return Err(format!("\"{canonical}\" is already assigned to \"{conflict}\"."));
    }

    let previous = bindings[index].clone();
    if previous.enabled {
        let _ = app.global_shortcut().unregister(previous.shortcut.as_str());
    }

    if enabled {
        if let Err(err) = register_with_handler(&app, &id, &name, &canonical) {
            if previous.enabled {
                let _ = register_with_handler(&app, &previous.id, &previous.name, &previous.shortcut);
            }
            return Err(err);
        }
    }

    let updated = HotkeyBinding {
        id,
        name,
        shortcut: canonical,
        enabled,
    };
    bindings[index] = updated.clone();
    save_bindings(&app, &bindings)?;
    Ok(updated)
}

#[tauri::command]
pub fn remove_hotkey(app: AppHandle, state: State<'_, HotkeyState>, id: String) -> Result<(), String> {
    let mut bindings = state.bindings.lock().unwrap();
    let index = bindings
        .iter()
        .position(|b| b.id == id)
        .ok_or("Hotkey not found.")?;
    let removed = bindings.remove(index);
    if removed.enabled {
        let _ = app.global_shortcut().unregister(removed.shortcut.as_str());
    }
    save_bindings(&app, &bindings)?;
    Ok(())
}

fn require_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim().to_string();
    if trimmed.is_empty() {
        return Err("Name is required.".into());
    }
    Ok(trimmed)
}

fn parse_and_validate(shortcut: &str) -> Result<Shortcut, String> {
    let parsed: Shortcut = shortcut
        .parse()
        .map_err(|_| format!("\"{shortcut}\" isn't a recognized shortcut, e.g. \"CommandOrControl+Shift+K\"."))?;
    if parsed.mods.is_empty() {
        return Err(
            "Add at least one modifier (Ctrl, Alt, Shift, or Win) so it doesn't hijack normal typing.".into(),
        );
    }
    Ok(parsed)
}

fn shortcut_matches(binding: &HotkeyBinding, id: u32) -> bool {
    binding
        .shortcut
        .parse::<Shortcut>()
        .map(|s| s.id() == id)
        .unwrap_or(false)
}

fn register_with_handler(app: &AppHandle, id: &str, name: &str, shortcut: &str) -> Result<(), String> {
    let id_owned = id.to_string();
    let name_owned = name.to_string();
    let shortcut_owned = shortcut.to_string();
    let app_handle = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                record_trigger(&app_handle, &id_owned, &name_owned, &shortcut_owned);
            }
        })
        .map_err(|e| e.to_string())
}

fn record_trigger(app: &AppHandle, id: &str, name: &str, shortcut: &str) {
    let entry = TriggerLogEntry {
        id: id.to_string(),
        name: name.to_string(),
        shortcut: shortcut.to_string(),
        at: now_millis(),
    };
    if let Some(state) = app.try_state::<HotkeyState>() {
        let mut log = state.log.lock().unwrap();
        log.push(entry.clone());
        if log.len() > MAX_LOG_ENTRIES {
            let excess = log.len() - MAX_LOG_ENTRIES;
            log.drain(0..excess);
        }
    }
    let _ = app.emit(TRIGGERED_EVENT, entry);
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn generate_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("hk_{}_{n}", now_millis())
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(CONFIG_FILE))
}

fn load_bindings(app: &AppHandle) -> Vec<HotkeyBinding> {
    config_path(app)
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

fn save_bindings(app: &AppHandle, bindings: &[HotkeyBinding]) -> Result<(), String> {
    let path = config_path(app)?;
    let json = serde_json::to_string_pretty(bindings).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}
