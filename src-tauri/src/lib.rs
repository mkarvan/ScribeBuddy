mod commands;
mod state;
mod tray;

use state::AppState;
use tauri::{Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Setup system tray
            if let Err(e) = tray::setup_tray(app.handle()) {
                log::error!("Failed to setup tray: {}", e);
            }

            // Resolve platform app data dir (~/Library/Application Support/<bundle-id> on macOS)
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("Cannot resolve app data directory");
            std::fs::create_dir_all(&data_dir)
                .expect("Cannot create app data directory");

            let config_path = data_dir.join("config.json");
            let config = AppState::load_config(&config_path);
            app.manage(AppState::new(config, config_path));

            log::info!("ScribeBuddy initialized");
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event {
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_running_apps,
            commands::list_audio_devices,
            commands::get_session_state,
            commands::set_target_app,
            commands::set_model_size,
            commands::set_chunk_duration,
            commands::set_capture_mode,
            commands::get_config,
            commands::check_model_available,
            commands::download_model,
            commands::start_session,
            commands::pause_session,
            commands::resume_session,
            commands::stop_session,
            commands::get_transcript,
            commands::export_markdown,
            commands::export_markdown_to_file,
            commands::set_language,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ScribeBuddy");
}
