use std::{
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow};

const TOAST_LABEL: &str = "toast";
const SHOW_EVENT: &str = "toast://show";
/// Gap between the notification and the screen edges it tucks into.
const EDGE_MARGIN: i32 = 12;
/// How long the notification stays on screen.
const VISIBLE_FOR: Duration = Duration::from_secs(2);

/// Identifies the most recent toast, so an earlier one's dismissal timer
/// can't hide a newer message that replaced it.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Flashes a short message in a small window above the system tray. Used for
/// confirmations that must be visible even when every FlowKeys window is
/// closed or minimized to the tray.
pub fn show(app: &AppHandle, message: String) {
    let Some(window) = app.get_webview_window(TOAST_LABEL) else {
        return;
    };
    position_bottom_right(&window);
    // Clicks must pass straight through to whatever is underneath; a
    // confirmation that swallows a click is worse than no confirmation.
    let _ = window.set_ignore_cursor_events(true);
    let _ = window.show();
    // Never call set_focus here: stealing focus mid-typing would be worse
    // than showing no confirmation at all.
    let _ = app.emit_to(TOAST_LABEL, SHOW_EVENT, message);

    // Dismissal is driven from here rather than from a timer in the webview,
    // so a message the webview never received still can't leave the window
    // stranded on screen.
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(VISIBLE_FOR);
        if GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }
        if let Some(window) = app.get_webview_window(TOAST_LABEL) {
            let _ = window.hide();
        }
    });
}

fn position_bottom_right(window: &WebviewWindow) {
    // A hidden window may not report a current monitor yet, so fall back to
    // the primary one.
    let monitor = match window.current_monitor() {
        Ok(Some(monitor)) => Some(monitor),
        _ => window.primary_monitor().ok().flatten(),
    };
    let Some(monitor) = monitor else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    // The work area excludes the taskbar, so the toast sits above it rather
    // than behind it.
    let area = monitor.work_area();
    let x = area.position.x + area.size.width as i32 - size.width as i32 - EDGE_MARGIN;
    let y = area.position.y + area.size.height as i32 - size.height as i32 - EDGE_MARGIN;
    let _ = window.set_position(PhysicalPosition::new(x, y));
}
