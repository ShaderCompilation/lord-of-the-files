//! Thin Tauri command handlers. All real logic lives in `engine`, `fs_scan`, and
//! `history`; these just adapt types and lock shared state.

use tauri::State;

use crate::ai;
use crate::engine;
use crate::fs_scan;
use crate::history::{self, ApplyReport, HistoryDb, Operation, RenameItem, UndoReport};
use crate::types::{AiResultItem, FileEntry, Pipeline, PreviewResult};

#[tauri::command]
pub fn scan_paths(paths: Vec<String>, recursive: bool, include_dirs: bool) -> Vec<FileEntry> {
    fs_scan::scan_paths(&paths, recursive, include_dirs)
}

#[tauri::command]
pub fn compute_preview(entries: Vec<FileEntry>, pipeline: Pipeline) -> PreviewResult {
    engine::compute_preview(&entries, &pipeline)
}

#[tauri::command]
pub fn apply_rename(db: State<HistoryDb>, items: Vec<RenameItem>) -> Result<ApplyReport, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(history::apply_rename(&conn, &items))
}

#[tauri::command]
pub fn list_operations(db: State<HistoryDb>) -> Result<Vec<Operation>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    history::list_operations(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn undo_operation(db: State<HistoryDb>, op_id: String) -> Result<UndoReport, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    history::undo_operation(&conn, &op_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn redo_operation(db: State<HistoryDb>, op_id: String) -> Result<UndoReport, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    history::redo_operation(&conn, &op_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ai_generate(
    prompt: String,
    entries: Vec<FileEntry>,
    max_len: u32,
) -> Result<Vec<AiResultItem>, String> {
    ai::generate(prompt, entries, max_len).await
}
