use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{de::DeserializeOwned, Serialize};
use tauri::{AppHandle, Manager};

/// Shared JSON-file persistence for per-feature config/state (hotkeys, clipboard
/// history, ...), all stored as sibling files under the app's config dir.
fn config_path(app: &AppHandle, file_name: &str) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(file_name))
}

pub fn load_json<T: DeserializeOwned + Default>(app: &AppHandle, file_name: &str) -> T {
    config_path(app, file_name)
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

pub fn save_json<T: Serialize>(app: &AppHandle, file_name: &str, value: &T) -> Result<(), String> {
    let path = config_path(app, file_name)?;
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// A unique-enough id for locally generated records: `{prefix}_{timestamp}_{counter}`.
pub fn generate_id(prefix: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{}_{n}", now_millis())
}
