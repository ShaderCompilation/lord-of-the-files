//! Shared data types that cross the Tauri IPC boundary.
//!
//! Everything here is `serde(rename_all = "camelCase")` so the TypeScript side can use
//! idiomatic camelCase. These types are mirrored (by hand) in `src/lib/types.ts`.

use serde::{Deserialize, Serialize};

/// A single file (or directory) selected for renaming.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    /// Stable id used to correlate rows across preview/apply (we use the path).
    pub id: String,
    /// Absolute path on disk.
    pub path: String,
    /// Absolute path of the containing directory.
    pub parent_dir: String,
    /// Filename without the extension (e.g. `archive.tar` for `archive.tar.gz`).
    pub stem: String,
    /// Extension without the leading dot (`gz`), or empty string when there is none.
    pub ext: String,
    pub is_dir: bool,
    pub size: u64,
    /// Modified time as unix seconds, if available.
    pub modified: Option<i64>,
}

/// Which part of the filename a step operates on.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    /// The name without extension (default — honours "preserve extension").
    #[default]
    Stem,
    /// The extension only.
    Ext,
    /// The whole filename including extension.
    Full,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CaseMode {
    Lower,
    Upper,
    Title,
    Sentence,
    Camel,
    Snake,
    Kebab,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InsertPosition {
    Prefix,
    Suffix,
    AtIndex,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RemoveFrom {
    Start,
    End,
    Index,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AffixPosition {
    Prefix,
    Suffix,
}

/// Result item cached on an AI step (filled in after the user clicks "Generate").
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AiResultItem {
    pub id: String,
    pub new_name: String,
}

/// Outcome of an `ai_generate` call: possibly-partial results plus batch accounting.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AiGenerateReport {
    pub results: Vec<AiResultItem>,
    pub failed_chunks: u32,
    pub total_chunks: u32,
    pub warning: Option<String>,
}

/// Emitted during `ai::generate` so the frontend can render live progress. Purely
/// informational — `AiGenerateReport` (the command's return value) remains the source of truth
/// for final results. Three variants cover the lifecycle of a generation: `Started` fires once
/// upfront (so the frontend learns `total_chunks` immediately instead of waiting on the first
/// chunk to finish), `ChunkStarted` fires when a chunk is actually dispatched (i.e. when
/// `buffer_unordered` polls its future), and `ChunkDone` fires when a chunk resolves.
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AiProgressEvent {
    Started {
        generation_id: String,
        total_chunks: u32,
    },
    ChunkStarted {
        generation_id: String,
        chunk_index: u32,
    },
    ChunkDone {
        generation_id: String,
        chunk_index: u32,
        total_chunks: u32,
        chunk_ok: bool,
        chunk_error: Option<String>,
        chunk_result_count: u32,
    },
}

/// The transform variants. Internally tagged by `type` so the TS union is ergonomic.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Step {
    FindReplace {
        find: String,
        replace: String,
        case_sensitive: bool,
        all_occurrences: bool,
    },
    Regex {
        pattern: String,
        replacement: String,
        ignore_case: bool,
        dotall: bool,
        multiline: bool,
    },
    ChangeCase {
        mode: CaseMode,
    },
    Insert {
        text: String,
        position: InsertPosition,
        /// Character index used when `position == atIndex`.
        index: i64,
    },
    Remove {
        from: RemoveFrom,
        /// Number of characters to remove.
        count: usize,
        /// Character index used when `from == index`.
        index: usize,
    },
    CleanUp {
        trim: bool,
        collapse_whitespace: bool,
        /// When set, spaces are replaced with this string (e.g. "-" or "_").
        spaces_to: Option<String>,
        strip_diacritics: bool,
    },
    Counter {
        start: i64,
        step: i64,
        /// Zero-padding width (e.g. 3 -> `007`).
        padding: usize,
        separator: String,
        position: AffixPosition,
        reset_per_directory: bool,
    },
    Ai {
        prompt: String,
        /// Cached results from the backend, keyed by file id at apply time.
        #[serde(default)]
        results: Option<Vec<AiResultItem>>,
    },
}

/// One configured step in the pipeline.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StepConfig {
    pub id: String,
    pub enabled: bool,
    #[serde(default)]
    pub scope: Scope,
    #[serde(flatten)]
    pub step: Step,
}

/// An ordered list of steps applied in sequence.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Pipeline {
    pub steps: Vec<StepConfig>,
}

/// Status of a single previewed rename.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RowStatus {
    Unchanged,
    Changed,
    Conflict,
    Invalid,
}

/// A single row of the two-column preview.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRow {
    pub id: String,
    pub original: String,
    pub new_name: String,
    pub status: RowStatus,
    /// Human-readable explanation for Conflict/Invalid rows.
    pub message: Option<String>,
}

/// An error attached to a specific step (e.g. an invalid regex pattern).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StepError {
    pub step_id: String,
    pub message: String,
}

/// The full result of `compute_preview`.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResult {
    pub rows: Vec<PreviewRow>,
    pub step_errors: Vec<StepError>,
}
