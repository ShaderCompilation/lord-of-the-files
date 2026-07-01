//! Thin Tauri command handlers. All real logic lives in `engine`, `fs_scan`, `history`,
//! `settings`, and `ai`; these just adapt types and lock shared state.

use std::sync::Arc;

use tauri::{Emitter, State};

use crate::ai;
use crate::engine;
use crate::fs_scan;
use crate::history::{
    self, ApplyReport, FileCheck, HistoryDb, Operation, RenameEntry, RenameItem, UndoReport,
};
use crate::settings::{self, MockAiConfig, ProviderProfile, SettingsDb, SettingsState};
use crate::types::{AiGenerateReport, AiProgressEvent, FileEntry, Pipeline, PreviewResult};

/// Broadcasts `ai::generate`'s per-chunk progress to the frontend via a Tauri event. Kept
/// here (not in `ai.rs`) so the `ai` module stays free of any Tauri-specific types.
struct TauriProgressEmitter(tauri::AppHandle);

impl ai::AiProgressEmitter for TauriProgressEmitter {
    fn emit(&self, event: AiProgressEvent) {
        if let Err(e) = self.0.emit("ai-generate-progress", event) {
            log::warn!("ai-generate-progress emit failed: {e}");
        }
    }
}

#[tauri::command]
pub fn scan_paths(paths: Vec<String>, recursive: bool, include_dirs: bool) -> Vec<FileEntry> {
    log::info!(
        "scan_paths: {} path(s), recursive={recursive}, include_dirs={include_dirs}",
        paths.len()
    );
    let entries = fs_scan::scan_paths(&paths, recursive, include_dirs);
    log::debug!("scan_paths: found {} entries", entries.len());
    entries
}

#[tauri::command]
pub fn compute_preview(entries: Vec<FileEntry>, pipeline: Pipeline) -> PreviewResult {
    engine::compute_preview(&entries, &pipeline)
}

#[tauri::command]
pub fn apply_rename(db: State<HistoryDb>, items: Vec<RenameItem>) -> Result<ApplyReport, String> {
    log::info!("apply_rename: {} item(s)", items.len());
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let report = history::apply_rename(&conn, &items);
    for item in &items {
        log::debug!("apply_rename: {} -> {}", item.old_path, item.new_name);
    }
    for f in &report.failures {
        log::warn!("apply_rename failed for {}: {}", f.path, f.error);
    }
    Ok(report)
}

#[tauri::command]
pub fn list_operations(db: State<HistoryDb>) -> Result<Vec<Operation>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    history::list_operations(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn undo_operation(db: State<HistoryDb>, op_id: String) -> Result<UndoReport, String> {
    log::info!("undo_operation: {op_id}");
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let report = history::undo_operation(&conn, &op_id).map_err(|e| e.to_string())?;
    for f in &report.failures {
        log::warn!("undo_operation failed for {}: {}", f.path, f.error);
    }
    Ok(report)
}

#[tauri::command]
pub fn redo_operation(db: State<HistoryDb>, op_id: String) -> Result<UndoReport, String> {
    log::info!("redo_operation: {op_id}");
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let report = history::redo_operation(&conn, &op_id).map_err(|e| e.to_string())?;
    for f in &report.failures {
        log::warn!("redo_operation failed for {}: {}", f.path, f.error);
    }
    Ok(report)
}

#[tauri::command]
pub fn get_operation_files(db: State<HistoryDb>, op_id: String) -> Result<Vec<RenameEntry>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    history::get_operation_files(&conn, &op_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn preview_undo(db: State<HistoryDb>, op_id: String) -> Result<Vec<FileCheck>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    history::preview_undo(&conn, &op_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn preview_redo(db: State<HistoryDb>, op_id: String) -> Result<Vec<FileCheck>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    history::preview_redo(&conn, &op_id).map_err(|e| e.to_string())
}

/// Looks up the active profile, erroring with a message pointing at Settings when none is
/// configured. A profile with no stored key (e.g. a local Ollama server) is still valid —
/// only the complete absence of an active profile is treated as "not set up".
fn active_profile(state: &SettingsState) -> Result<ProviderProfile, String> {
    let active_id = state
        .active_profile_id
        .as_deref()
        .ok_or_else(|| "No active provider / no key — open Settings".to_string())?;
    state
        .profiles
        .iter()
        .find(|p| p.id == active_id)
        .cloned()
        .ok_or_else(|| "No active provider / no key — open Settings".to_string())
}

#[tauri::command]
pub async fn ai_generate(
    app: tauri::AppHandle,
    db: State<'_, SettingsDb>,
    prompt: String,
    entries: Vec<FileEntry>,
    generation_id: String,
) -> Result<AiGenerateReport, String> {
    let (profile, mock) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let state = settings::load_state(&conn);
        let profile = active_profile(&state)?;
        let mock = state.mock_ai.enabled.then(|| state.mock_ai.clone());
        (profile, mock)
    };
    let key = settings::get_api_key(&profile.id).unwrap_or_default();
    let has_key = !key.is_empty();
    log::info!(
        "ai_generate: generation_id={generation_id}, {} entries, profile_id={}, label={}, has_key={has_key}, mock={}, prompt_len={}",
        entries.len(),
        profile.id,
        profile.label,
        mock.is_some(),
        prompt.len()
    );
    log::trace!("ai_generate: prompt={prompt}");
    let emitter: Arc<dyn ai::AiProgressEmitter> = Arc::new(TauriProgressEmitter(app));
    ai::generate(&profile, &key, prompt, entries, &generation_id, emitter, mock).await
}

#[tauri::command]
pub fn get_settings(db: State<SettingsDb>) -> Result<SettingsState, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut state = settings::load_state(&conn);
    for profile in &mut state.profiles {
        profile.has_key = settings::has_api_key(&profile.id);
    }
    Ok(state)
}

#[tauri::command]
pub fn upsert_profile(db: State<SettingsDb>, profile: ProviderProfile) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut state = settings::load_state(&conn);
    match state.profiles.iter_mut().find(|p| p.id == profile.id) {
        Some(existing) => *existing = profile,
        None => state.profiles.push(profile),
    }
    settings::save_state(&conn, &state)
}

#[tauri::command]
pub fn delete_profile(db: State<SettingsDb>, id: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut state = settings::load_state(&conn);
    state.profiles.retain(|p| p.id != id);
    if state.active_profile_id.as_deref() == Some(id.as_str()) {
        state.active_profile_id = None;
    }
    settings::save_state(&conn, &state)?;
    settings::clear_api_key(&id)
}

#[tauri::command]
pub fn set_active_profile(db: State<SettingsDb>, id: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut state = settings::load_state(&conn);
    if !state.profiles.iter().any(|p| p.id == id) {
        return Err(format!("No such profile: {id}"));
    }
    state.active_profile_id = Some(id);
    settings::save_state(&conn, &state)
}

#[tauri::command]
pub fn set_api_key(profile_id: String, key: String) -> Result<(), String> {
    settings::set_api_key(&profile_id, &key)
}

#[tauri::command]
pub fn clear_api_key(profile_id: String) -> Result<(), String> {
    settings::clear_api_key(&profile_id)
}

#[tauri::command]
pub fn set_debug_logging(db: State<SettingsDb>, enabled: bool) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut state = settings::load_state(&conn);
    state.debug_logging = enabled;
    settings::save_state(&conn, &state)?;
    crate::logging::set_debug(enabled);
    log::info!("debug logging {}", if enabled { "enabled" } else { "disabled" });
    Ok(())
}

/// Dev-menu-only: persists the "Mock AI" config. The setting is always stored so it survives
/// restarts within a dev build, but `ai::generate` only ever acts on it under
/// `cfg!(debug_assertions)` — a release build ignores it regardless of what's in settings.db.
#[tauri::command]
pub fn set_mock_ai_config(db: State<SettingsDb>, config: MockAiConfig) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut state = settings::load_state(&conn);
    state.mock_ai = config;
    settings::save_state(&conn, &state)?;
    log::info!(
        "set_mock_ai_config: enabled={}, latency_ms={}, fail_rate={}, transform={:?}",
        state.mock_ai.enabled,
        state.mock_ai.latency_ms,
        state.mock_ai.fail_rate,
        state.mock_ai.transform
    );
    Ok(())
}

#[tauri::command]
pub async fn test_connection(db: State<'_, SettingsDb>, profile_id: String) -> Result<String, String> {
    log::info!("test_connection: profile_id={profile_id}");
    let profile = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        settings::load_state(&conn)
            .profiles
            .into_iter()
            .find(|p| p.id == profile_id)
            .ok_or_else(|| format!("No such profile: {profile_id}"))?
    };
    let key = settings::get_api_key(&profile.id).unwrap_or_default();
    log::debug!(
        "test_connection: base_url={}, model={}, has_key={}",
        profile.base_url,
        profile.model,
        !key.is_empty()
    );
    let entries = vec![FileEntry {
        id: "test".to_string(),
        path: "/test/file.txt".to_string(),
        parent_dir: "/test".to_string(),
        stem: "file".to_string(),
        ext: "txt".to_string(),
        is_dir: false,
        size: 0,
        modified: None,
    }];
    ai::generate(
        &profile,
        &key,
        "Echo the original name".to_string(),
        entries,
        "test-connection",
        Arc::new(ai::NoopProgressEmitter),
        None, // always exercises the real provider, even if Mock AI is on
    )
    .await
    .map(|report| {
        log::info!(
            "test_connection: profile_id={profile_id} ok — {} result(s)",
            report.results.len()
        );
        "ok".to_string()
    })
    .map_err(|e| {
        log::warn!("test_connection: profile_id={profile_id} failed: {e}");
        e
    })
}
