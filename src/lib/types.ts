// Mirrors the serde types in src-tauri/src/types.rs (camelCase across the IPC boundary).

export interface FileEntry {
  id: string;
  path: string;
  parentDir: string;
  stem: string;
  ext: string;
  isDir: boolean;
  size: number;
  modified: number | null;
}

export type Scope = "stem" | "ext" | "full";
export type CaseMode =
  | "lower"
  | "upper"
  | "title"
  | "sentence"
  | "camel"
  | "snake"
  | "kebab";
export type InsertPosition = "prefix" | "suffix" | "atIndex";
export type RemoveFrom = "start" | "end" | "index";
export type AffixPosition = "prefix" | "suffix";

export interface AiResultItem {
  id: string;
  newName: string;
}

export type StepType =
  | "findReplace"
  | "regex"
  | "changeCase"
  | "insert"
  | "remove"
  | "cleanUp"
  | "counter"
  | "ai";

export type Step =
  | {
      type: "findReplace";
      find: string;
      replace: string;
      caseSensitive: boolean;
      allOccurrences: boolean;
    }
  | {
      type: "regex";
      pattern: string;
      replacement: string;
      ignoreCase: boolean;
      dotall: boolean;
      multiline: boolean;
    }
  | { type: "changeCase"; mode: CaseMode }
  | { type: "insert"; text: string; position: InsertPosition; index: number }
  | { type: "remove"; from: RemoveFrom; count: number; index: number }
  | {
      type: "cleanUp";
      trim: boolean;
      collapseWhitespace: boolean;
      spacesTo: string | null;
      stripDiacritics: boolean;
    }
  | {
      type: "counter";
      start: number;
      step: number;
      padding: number;
      separator: string;
      position: AffixPosition;
      resetPerDirectory: boolean;
    }
  | { type: "ai"; prompt: string; results: AiResultItem[] | null };

export type StepConfig = { id: string; enabled: boolean; scope: Scope } & Step;

export interface Pipeline {
  steps: StepConfig[];
}

export type RowStatus = "unchanged" | "changed" | "conflict" | "invalid";

export interface PreviewRow {
  id: string;
  original: string;
  newName: string;
  status: RowStatus;
  message: string | null;
}

export interface StepError {
  stepId: string;
  message: string;
}

export interface PreviewResult {
  rows: PreviewRow[];
  stepErrors: StepError[];
}

export interface RenameItem {
  oldPath: string;
  newName: string;
}

export interface Failure {
  path: string;
  error: string;
}

export interface ApplyReport {
  operationId: string | null;
  renamed: number;
  failures: Failure[];
  historyError: string | null;
}

export interface UndoReport {
  reverted: number;
  failures: Failure[];
}

export interface Operation {
  id: string;
  createdAt: string;
  summary: string;
  itemCount: number;
  status: "applied" | "undone" | "partial";
}

export interface RenameEntry {
  oldPath: string;
  newPath: string;
  status: "applied" | "undone";
}

export type CheckStatus = "ok" | "missing" | "would-overwrite";

export interface FileCheck {
  oldPath: string;
  newPath: string;
  status: CheckStatus;
}

// BYOK provider settings (see src-tauri/src/settings.rs).
export interface ProviderProfile {
  id: string;
  label: string;
  baseUrl: string;
  model: string;
  chunkSize: number;
  concurrency: number;
  maxLen: number;
  timeoutSecs: number;
  hasKey: boolean;
}

// Dev menu: simulated AI backend (see src-tauri/src/settings.rs::MockAiConfig). Persisted, but
// only ever honoured by the Rust side in debug builds.
export type MockTransform = "suffix" | "uppercase" | "lowercase" | "reverse" | "slugify";

export interface MockAiConfig {
  enabled: boolean;
  latencyMs: number;
  /** 0-1 chance that any given chunk simulates a provider failure. */
  failRate: number;
  transform: MockTransform;
}

export interface SettingsState {
  profiles: ProviderProfile[];
  activeProfileId: string | null;
  debugLogging: boolean;
  mockAi: MockAiConfig;
}

export interface AiGenerateReport {
  results: AiResultItem[];
  failedChunks: number;
  totalChunks: number;
  warning: string | null;
}

export type AiProgressEvent =
  | { kind: "started"; generationId: string; totalChunks: number }
  | { kind: "chunkStarted"; generationId: string; chunkIndex: number }
  | {
      kind: "chunkDone";
      generationId: string;
      chunkIndex: number;
      totalChunks: number;
      chunkOk: boolean;
      chunkError: string | null;
      chunkResultCount: number;
    };
