use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::clipboard;
use crate::support::{generate_id, load_json, now_millis, save_json};

const CONFIG_FILE: &str = "hotkeys.json";
const MAX_LOG_ENTRIES: usize = 50;
const TRIGGERED_EVENT: &str = "hotkeys://triggered";
const DEFAULT_POPUP_SHORTCUT: &str = "Alt+Shift+V";

/// What a hotkey does when pressed, beyond just showing up in the trigger log.
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyAction {
    /// User-defined, no built-in behavior (yet) — just fires the trigger event.
    Custom,
    /// Opens/closes the floating clipboard popup.
    ToggleClipboardPopup,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HotkeyBinding {
    pub id: String,
    pub name: String,
    /// Canonical form produced by the shortcut parser, e.g. "control+shift+keyk".
    pub shortcut: String,
    pub enabled: bool,
    #[serde(default = "default_action")]
    pub action: HotkeyAction,
}

fn default_action() -> HotkeyAction {
    HotkeyAction::Custom
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
    let mut bindings: Vec<HotkeyBinding> = load_json(app, CONFIG_FILE);
    ensure_builtin_hotkeys(&mut bindings);

    for binding in bindings.iter_mut() {
        if !binding.enabled {
            continue;
        }
        if let Err(err) = register_with_handler(app, binding) {
            eprintln!(
                "FlowKeys: couldn't re-register hotkey \"{}\" ({}): {err}",
                binding.name, binding.shortcut
            );
            binding.enabled = false;
        }
    }
    save_json(app, CONFIG_FILE, &bindings)?;

    app.manage(HotkeyState {
        bindings: Mutex::new(bindings),
        log: Mutex::new(Vec::new()),
    });
    Ok(())
}

/// Seeds the built-in "Toggle Clipboard Popup" hotkey on first run. Once
/// present it's never re-seeded — `remove_hotkey` refuses to delete
/// non-`Custom` bindings, so it can only be disabled or rebound.
fn ensure_builtin_hotkeys(bindings: &mut Vec<HotkeyBinding>) {
    let has_popup_toggle = bindings.iter().any(|b| b.action == HotkeyAction::ToggleClipboardPopup);
    if has_popup_toggle {
        return;
    }
    if let Ok(parsed) = parse_and_validate(DEFAULT_POPUP_SHORTCUT) {
        bindings.push(HotkeyBinding {
            id: generate_id("hk"),
            name: "Toggle Clipboard Popup".to_string(),
            shortcut: parsed.into_string(),
            enabled: true,
            action: HotkeyAction::ToggleClipboardPopup,
        });
    }
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

    let binding = HotkeyBinding {
        id: generate_id("hk"),
        name,
        shortcut: canonical,
        enabled: true,
        action: HotkeyAction::Custom,
    };
    register_with_handler(&app, &binding)?;

    bindings.push(binding.clone());
    save_json(&app, CONFIG_FILE, &*bindings)?;
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

    let updated = HotkeyBinding {
        id,
        name,
        shortcut: canonical,
        enabled,
        action: previous.action.clone(),
    };

    if enabled {
        if let Err(err) = register_with_handler(&app, &updated) {
            if previous.enabled {
                let _ = register_with_handler(&app, &previous);
            }
            return Err(err);
        }
    }

    bindings[index] = updated.clone();
    save_json(&app, CONFIG_FILE, &*bindings)?;
    Ok(updated)
}

#[tauri::command]
pub fn remove_hotkey(app: AppHandle, state: State<'_, HotkeyState>, id: String) -> Result<(), String> {
    let mut bindings = state.bindings.lock().unwrap();
    let index = bindings
        .iter()
        .position(|b| b.id == id)
        .ok_or("Hotkey not found.")?;
    if bindings[index].action != HotkeyAction::Custom {
        return Err("Built-in hotkeys can't be removed — disable them instead.".into());
    }
    let removed = bindings.remove(index);
    if removed.enabled {
        let _ = app.global_shortcut().unregister(removed.shortcut.as_str());
    }
    save_json(&app, CONFIG_FILE, &*bindings)?;
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

fn register_with_handler(app: &AppHandle, binding: &HotkeyBinding) -> Result<(), String> {
    let id_owned = binding.id.clone();
    let name_owned = binding.name.clone();
    let shortcut_owned = binding.shortcut.clone();
    let action = binding.action.clone();
    let app_handle = app.clone();
    app.global_shortcut()
        .on_shortcut(binding.shortcut.as_str(), move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                record_trigger(&app_handle, &id_owned, &name_owned, &shortcut_owned);
                if action == HotkeyAction::ToggleClipboardPopup {
                    clipboard::toggle_popup(&app_handle);
                }
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
