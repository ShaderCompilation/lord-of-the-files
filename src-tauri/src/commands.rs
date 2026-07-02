//! Thin Tauri command handlers. All real logic lives in `engine`, `fs_scan`, `history`,
//! `settings`, and `ai`; these just adapt types and lock shared state.

use tauri::State;
use tokio_util::sync::CancellationToken;

use crate::ai;
use crate::ai_history::{self, AiGenerationDetail, AiGenerationSummary};
use crate::ai_registry::AiGenerationRegistry;
use crate::engine;
use crate::fs_scan;
use crate::history::{
    self, ApplyReport, FileCheck, HistoryDb, Operation, RenameEntry, RenameItem, UndoReport,
};
use crate::settings::{self, MockAiConfig, ProviderProfile, SettingsDb, SettingsState};
use crate::types::{AiGenerateReport, FileEntry, Pipeline, PreviewResult};

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
    settings_db: State<'_, SettingsDb>,
    history_db: State<'_, HistoryDb>,
    registry: State<'_, AiGenerationRegistry>,
    prompt: String,
    entries: Vec<FileEntry>,
    generation_id: String,
) -> Result<AiGenerateReport, String> {
    let (profile, mock) = {
        let conn = settings_db.0.lock().map_err(|e| e.to_string())?;
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
    let token = registry.register(generation_id.clone());
    let report = ai::generate(&profile, &key, prompt, entries, &generation_id, mock, token).await;
    registry.remove(&generation_id);

    {
        let conn = history_db.0.lock().map_err(|e| e.to_string())?;
        if let Err(e) = ai_history::record_generation(&conn, &report) {
            log::warn!("ai_generate: generation_id={generation_id} failed to record AI history: {e}");
        }
    }

    match report.error {
        Some(e) => Err(e),
        None => Ok(report),
    }
}

/// Best-effort cancellation of an in-flight `ai_generate` call. Not an error if the
/// generation already finished (or was never registered) — the frontend calls this
/// fire-and-forget whenever it stops caring about a generation's result (user clicked
/// Cancel, or a newer generation superseded it), and by the time this arrives the original
/// call may well have already completed on its own.
#[tauri::command]
pub fn cancel_ai_generate(
    registry: State<AiGenerationRegistry>,
    generation_id: String,
) -> Result<(), String> {
    log::info!("cancel_ai_generate: generation_id={generation_id}");
    registry.cancel(&generation_id);
    Ok(())
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
    let report = ai::generate(
        &profile,
        &key,
        "Echo the original name".to_string(),
        entries,
        "test-connection",
        None, // always exercises the real provider, even if Mock AI is on
        CancellationToken::new(),
    )
    .await;

    // Not persisted to AI History: this is a synthetic connectivity probe against a fake file,
    // not a real user request.
    match report.error {
        None => {
            log::info!(
                "test_connection: profile_id={profile_id} ok — {} result(s)",
                report.results.len()
            );
            Ok("ok".to_string())
        }
        Some(e) => {
            log::warn!("test_connection: profile_id={profile_id} failed: {e}");
            Err(e)
        }
    }
}

#[tauri::command]
pub fn list_ai_generations(db: State<HistoryDb>) -> Result<Vec<AiGenerationSummary>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    ai_history::list_ai_generations(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_ai_generation(db: State<HistoryDb>, id: String) -> Result<Option<AiGenerationDetail>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    ai_history::get_ai_generation(&conn, &id).map_err(|e| e.to_string())
}
