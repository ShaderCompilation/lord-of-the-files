// Typed wrappers around the Tauri command surface. Tauri v2 maps camelCase JS argument
// keys to the snake_case Rust parameters automatically.

import { invoke } from "@tauri-apps/api/core";

import type {
  AiResultItem,
  ApplyReport,
  FileEntry,
  Operation,
  Pipeline,
  PreviewResult,
  RenameItem,
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

export function aiGenerate(
  prompt: string,
  entries: FileEntry[],
  maxLen: number,
): Promise<AiResultItem[]> {
  return invoke("ai_generate", { prompt, entries, maxLen });
}
