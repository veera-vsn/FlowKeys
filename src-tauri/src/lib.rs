mod hotkeys;

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, WindowEvent,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            hotkeys::list_hotkeys,
            hotkeys::recent_triggers,
            hotkeys::add_hotkey,
            hotkeys::update_hotkey,
            hotkeys::remove_hotkey,
        ])
        .setup(|app| {
            let show_settings = MenuItem::with_id(app, "show_settings", "Open Settings", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit FlowKeys", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_settings, &quit])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("FlowKeys")
                .menu(&tray_menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show_settings" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            hotkeys::init(app.handle())?;

            Ok(())
        })
        .on_window_event(|window, event| {
            // Close-to-tray: keep the process (and hotkeys/clipboard watching)
            // running in the background instead of quitting on window close.
            if let WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
