mod ai;
mod commands;
mod engine;
mod fs_scan;
mod history;
mod types;

#[cfg(test)]
mod integration_tests;

use std::sync::Mutex;

use rusqlite::Connection;
use tauri::Manager;

use history::HistoryDb;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Open (or create) the history database in the app data directory.
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = Connection::open(dir.join("history.db"))?;
            history::init_schema(&conn)?;
            app.manage(HistoryDb(Mutex::new(conn)));
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
