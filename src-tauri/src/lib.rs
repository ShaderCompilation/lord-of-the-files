mod ai;
mod commands;
mod engine;
mod fs_scan;
mod history;
mod logging;
mod settings;
mod types;

#[cfg(test)]
mod integration_tests;

use std::sync::Mutex;

use rusqlite::Connection;
use tauri::Manager;

use history::HistoryDb;
use settings::SettingsDb;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(logging::plugin())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Open (or create) the history database in the app data directory.
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = Connection::open(dir.join("history.db"))?;
            history::init_schema(&conn)?;
            app.manage(HistoryDb(Mutex::new(conn)));

            let settings_conn = Connection::open(dir.join("settings.db"))?;
            settings::init_schema(&settings_conn)?;
            let st = settings::load_state(&settings_conn);
            logging::set_debug(st.debug_logging);
            log::info!("started; debug_logging={}", st.debug_logging);
            app.manage(SettingsDb(Mutex::new(settings_conn)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan_paths,
            commands::compute_preview,
            commands::apply_rename,
            commands::list_operations,
            commands::undo_operation,
            commands::redo_operation,
            commands::ai_generate,
            commands::get_settings,
            commands::upsert_profile,
            commands::delete_profile,
            commands::set_active_profile,
            commands::set_api_key,
            commands::clear_api_key,
            commands::test_connection,
            commands::set_debug_logging,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
