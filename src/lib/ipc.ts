// Typed wrappers around the Tauri command surface. Tauri v2 maps camelCase JS argument
// keys to the snake_case Rust parameters automatically.

import { invoke } from "@tauri-apps/api/core";

import type {
  AiGenerateReport,
  ApplyReport,
  FileEntry,
  Operation,
  Pipeline,
  PreviewResult,
  ProviderProfile,
  RenameItem,
  SettingsState,
  UndoReport,
} from "./types";

export function scanPaths(
  paths: string[],
  recursive: boolean,
  includeDirs: boolean,
): Promise<FileEntry[]> {
  return invoke("scan_paths", { paths, recursive, includeDirs });
}

export function computePreview(
  entries: FileEntry[],
  pipeline: Pipeline,
): Promise<PreviewResult> {
  return invoke("compute_preview", { entries, pipeline });
}

export function applyRename(items: RenameItem[]): Promise<ApplyReport> {
  return invoke("apply_rename", { items });
}

export function listOperations(): Promise<Operation[]> {
  return invoke("list_operations");
}

export function undoOperation(opId: string): Promise<UndoReport> {
  return invoke("undo_operation", { opId });
}

export function redoOperation(opId: string): Promise<UndoReport> {
  return invoke("redo_operation", { opId });
}

export function aiGenerate(prompt: string, entries: FileEntry[]): Promise<AiGenerateReport> {
  return invoke("ai_generate", { prompt, entries });
}

export function getSettings(): Promise<SettingsState> {
  return invoke("get_settings");
}

export function upsertProfile(profile: ProviderProfile): Promise<void> {
  return invoke("upsert_profile", { profile });
}

export function deleteProfile(id: string): Promise<void> {
  return invoke("delete_profile", { id });
}

export function setActiveProfile(id: string): Promise<void> {
  return invoke("set_active_profile", { id });
}

export function setApiKey(profileId: string, key: string): Promise<void> {
  return invoke("set_api_key", { profileId, key });
}

export function clearApiKey(profileId: string): Promise<void> {
  return invoke("clear_api_key", { profileId });
}

export function testConnection(profileId: string): Promise<string> {
  return invoke("test_connection", { profileId });
}

export function setDebugLogging(enabled: boolean): Promise<void> {
  return invoke("set_debug_logging", { enabled });
}
