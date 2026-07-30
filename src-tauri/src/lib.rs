mod clipboard;
mod hotkeys;
mod settings;
mod snippets;
mod support;
mod toast;

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
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            hotkeys::list_hotkeys,
            hotkeys::recent_triggers,
            hotkeys::add_hotkey,
            hotkeys::update_hotkey,
            hotkeys::remove_hotkey,
            clipboard::list_clipboard_history,
            clipboard::copy_clipboard_entry,
            clipboard::remove_clipboard_entry,
            clipboard::clear_clipboard_history,
            clipboard::hide_clipboard_popup,
            snippets::list_snippets,
            snippets::add_snippet,
            snippets::update_snippet,
            snippets::remove_snippet,
            settings::get_settings,
            settings::set_copy_on_selection,
        ])
        .setup(|app| {
            let show_settings = MenuItem::with_id(app, "show_settings", "Open Settings", true, None::<&str>)?;
            let open_clipboard = MenuItem::with_id(app, "open_clipboard", "Open Clipboard", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit FlowKeys", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_settings, &open_clipboard, &quit])?;

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
                    "open_clipboard" => clipboard::toggle_popup(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            settings::init(app.handle())?;
            hotkeys::init(app.handle())?;
            clipboard::init(app.handle())?;
            snippets::init(app.handle())?;

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
