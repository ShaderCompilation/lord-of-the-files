//! BYOK AI rename: dispatches chunks of files to a user-configured OpenAI-compatible
//! endpoint via the `aisdk` crate, and lenient-parses the JSON the model returns.
//!
//! No `response_format`/structured-output is ever requested (see
//! `docs/byok-ai-rename-plan.md`), so providers/models that would hard-reject that field
//! still work — we rely entirely on `extract_results` to recover JSON from a plain text
//! response (fenced in ```json, wrapped in prose, or clean).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aisdk::core::{DynamicModel, LanguageModelRequest};
use aisdk::providers::OpenAICompatible;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::settings::{MockAiConfig, MockTransform, ProviderProfile};
use crate::types::{AiGenerateReport, AiProgressEvent, AiResultItem, FileEntry};

/// Placeholder sent when a profile has no key configured (e.g. local Ollama / LM Studio),
/// since `aisdk`'s `OpenAICompatible` builder rejects an empty `api_key` even though these
/// local servers never check the `Authorization` header.
const NO_KEY_PLACEHOLDER: &str = "not-needed";

/// Seam between `ai::generate`'s per-chunk progress and however the caller wants to surface
/// it. Exists so unit tests (which run outside a Tauri `AppHandle`) can pass a no-op
/// implementation instead of needing a live Tauri app.
pub trait AiProgressEmitter: Send + Sync {
    fn emit(&self, event: AiProgressEvent);
}

pub struct NoopProgressEmitter;

impl AiProgressEmitter for NoopProgressEmitter {
    fn emit(&self, _event: AiProgressEvent) {}
}

#[allow(clippy::too_many_arguments)]
pub async fn generate(
    cfg: &ProviderProfile,
    api_key: &str,
    prompt: String,
    entries: Vec<FileEntry>,
    generation_id: &str,
    emitter: Arc<dyn AiProgressEmitter>,
    mock: Option<MockAiConfig>,
    cancel: CancellationToken,
) -> Result<AiGenerateReport, String> {
    // Belt-and-suspenders: a release binary never mocks, even if `enabled` was left on in
    // settings.db by a previous dev build, or a command were invoked directly.
    let mock = if cfg!(debug_assertions) { mock } else { None };

    let key = if api_key.is_empty() {
        NO_KEY_PLACEHOLDER
    } else {
        api_key
    };
    // Skip building a real client entirely when mocking — `cfg.base_url`/`model` don't need to
    // be valid (or even set) for a mocked generation.
    let provider = if mock.is_none() {
        Some(
            OpenAICompatible::<DynamicModel>::builder()
                .base_url(&cfg.base_url)
                .api_key(key)
                .model_name(&cfg.model)
                .build()
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };

    let chunk_size = (cfg.chunk_size as usize).max(1);
    let concurrency = (cfg.concurrency as usize).max(1);
    let timeout = Duration::from_secs(cfg.timeout_secs.max(1) as u64);
    let system = system_prompt(cfg.max_len);
    let entry_count = entries.len();

    if cancel.is_cancelled() {
        log::debug!("ai::generate: generation_id={generation_id} cancelled before dispatch");
        return Err("Cancelled".to_string());
    }

    let chunks = chunk_entries(entries, chunk_size);
    let total_chunks = chunks.len();
    let has_key = !api_key.is_empty();
    let mock_on = mock.is_some();

    log::info!(
        "ai::generate: generation_id={generation_id}, {entry_count} entries, {total_chunks} chunk(s), \
         profile_id={profile_id}, has_key={has_key}, base_url={base_url}, model={model}, \
         chunk_size={chunk_size}, concurrency={concurrency}, timeout_secs={timeout_secs}, max_len={max_len}, \
         mock={mock_on}",
        profile_id = cfg.id,
        base_url = cfg.base_url,
        model = cfg.model,
        timeout_secs = cfg.timeout_secs,
        max_len = cfg.max_len,
    );
    log::trace!("ai::generate: instruction={prompt}");
    log::trace!("ai::generate: system={system}");

    let tasks = chunks.into_iter().enumerate().map(|(chunk_index, chunk)| {
        let provider = provider.clone();
        let mock = mock.clone();
        let system = system.clone();
        let user = user_prompt(&prompt, &chunk, chunk_index * chunk_size);
        let ids: HashSet<String> = chunk.iter().map(|e| e.id.clone()).collect();
        let stems: HashMap<String, String> =
            chunk.iter().map(|e| (e.id.clone(), e.stem.clone())).collect();
        let exts: HashMap<String, String> =
            chunk.iter().map(|e| (e.id.clone(), e.ext.clone())).collect();
        let gen_id = generation_id.to_string();
        let emitter = Arc::clone(&emitter);
        let cancel = cancel.clone();
        async move {
            if cancel.is_cancelled() {
                log::debug!(
                    "ai::generate: generation_id={gen_id} chunk {chunk_index} skipped (already cancelled)"
                );
                emitter.emit(AiProgressEvent {
                    generation_id: gen_id,
                    chunk_index: chunk_index as u32,
                    total_chunks: total_chunks as u32,
                    chunk_ok: false,
                    chunk_error: Some("Cancelled".to_string()),
                    chunk_result_count: 0,
                });
                return (chunk_index, Err("Cancelled".to_string()));
            }

            log::debug!(
                "ai::generate: generation_id={gen_id} chunk {chunk_index}/{total_chunks} dispatching {} file(s)",
                chunk.len()
            );
            log::trace!(
                "ai::generate: generation_id={gen_id} chunk {chunk_index} user_prompt={user}"
            );

            let started = Instant::now();
            let outcome = match &mock {
                Some(mock_cfg) => run_mock_chunk(mock_cfg, &chunk, chunk_index, &gen_id).await,
                None => {
                    let provider = provider.expect("provider is built whenever mocking is off");
                    run_chunk(provider, system, user, timeout, chunk_index, &gen_id, &cancel).await
                }
            };
            let elapsed_ms = started.elapsed().as_millis();

            let outcome = outcome.map(|parsed| {
                let report = reconcile(parsed.items, &ids, &stems, &exts, &gen_id);
                log_chunk_reconcile(chunk_index, &gen_id, &report, &stems, parsed.parse_path, elapsed_ms);
                report.items
            });

            if let Err(e) = &outcome {
                log::warn!(
                    "ai::generate: generation_id={gen_id} chunk {chunk_index} failed after {elapsed_ms}ms: {e}"
                );
            }

            let chunk_ok = outcome.is_ok();
            let chunk_error = outcome.as_ref().err().cloned();
            let chunk_result_count = outcome.as_ref().map(|v| v.len() as u32).unwrap_or(0);
            emitter.emit(AiProgressEvent {
                generation_id: gen_id,
                chunk_index: chunk_index as u32,
                total_chunks: total_chunks as u32,
                chunk_ok,
                chunk_error,
                chunk_result_count,
            });

            (chunk_index, outcome)
        }
    });

    let mut by_chunk: Vec<(usize, Result<Vec<AiResultItem>, String>)> =
        stream::iter(tasks).buffer_unordered(concurrency).collect().await;
    by_chunk.sort_by_key(|(i, _)| *i);

    let mut results = Vec::new();
    let mut failed_chunks = 0u32;
    let mut first_error: Option<String> = None;
    for (_, outcome) in by_chunk {
        match outcome {
            Ok(items) => results.extend(items),
            Err(e) => {
                failed_chunks += 1;
                first_error.get_or_insert(e);
            }
        }
    }

    if total_chunks > 0 && failed_chunks as usize == total_chunks {
        let err = first_error.unwrap_or_else(|| "AI generation failed".to_string());
        log::warn!("ai::generate: generation_id={generation_id} all chunks failed: {err}");
        return Err(err);
    }

    let warning = (failed_chunks > 0).then(|| {
        format!(
            "Suggested {} name(s); {} of {} batch(es) failed.",
            results.len(),
            failed_chunks,
            total_chunks
        )
    });

    if let Some(ref w) = warning {
        log::warn!("ai::generate: generation_id={generation_id} partial success: {w}");
    }
    log::info!(
        "ai::generate: generation_id={generation_id} done — {} result(s), {failed_chunks}/{total_chunks} chunk(s) failed",
        results.len()
    );

    Ok(AiGenerateReport {
        results,
        failed_chunks,
        total_chunks: total_chunks as u32,
        warning,
    })
}

/// Split `entries` into contiguous, order-preserving, owned chunks of at most `size`. Chunks
/// are owned (not borrowed slices) so the per-chunk futures dispatched below are `'static` —
/// otherwise they'd tie the whole `buffer_unordered` stream to `entries`' stack lifetime,
/// which trips up the higher-ranked lifetime inference `#[tauri::command]`'s macro-generated
/// wrapper needs for `State<'_, _>` parameters.
fn chunk_entries(entries: Vec<FileEntry>, size: usize) -> Vec<Vec<FileEntry>> {
    if entries.is_empty() {
        return Vec::new();
    }
    entries.chunks(size.max(1)).map(<[FileEntry]>::to_vec).collect()
}

/// Outcome of a single attempt inside `run_chunk`, distinguishing "timed out" (retryable) from
/// "cancelled" (never retried) and other terminal failures, so the retry loop can decide
/// correctly without string-matching error messages.
enum AttemptOutcome {
    Success(ParsedChunk),
    TimedOut,
    Cancelled,
    Failed(String),
}

/// Runs one chunk to completion, retrying exactly once if (and only if) the attempt times
/// out — the log evidence that motivated this showed the same request succeeding well within
/// the timeout on a second try, so a single bounded retry can salvage transient slowness
/// without masking real errors (a bad key or malformed response won't change on retry).
async fn run_chunk(
    provider: OpenAICompatible<DynamicModel>,
    system: String,
    prompt: String,
    timeout: Duration,
    chunk_index: usize,
    generation_id: &str,
    cancel: &CancellationToken,
) -> Result<ParsedChunk, String> {
    const MAX_ATTEMPTS: u32 = 2; // one attempt + one retry, timeout-only

    for attempt_num in 1..=MAX_ATTEMPTS {
        match run_chunk_attempt(
            provider.clone(),
            system.clone(),
            prompt.clone(),
            timeout,
            chunk_index,
            generation_id,
            cancel,
        )
        .await
        {
            AttemptOutcome::Success(parsed) => return Ok(parsed),
            AttemptOutcome::Cancelled => {
                log::debug!(
                    "ai::generate: generation_id={generation_id} chunk {chunk_index} cancelled \
                     (generation superseded or user-cancelled)"
                );
                return Err("Cancelled".to_string());
            }
            AttemptOutcome::Failed(e) => return Err(e),
            AttemptOutcome::TimedOut if attempt_num < MAX_ATTEMPTS => {
                log::warn!(
                    "ai::generate: generation_id={generation_id} chunk {chunk_index} timed out \
                     after {}s, retrying once",
                    timeout.as_secs()
                );
            }
            AttemptOutcome::TimedOut => {
                log::warn!(
                    "ai::generate: generation_id={generation_id} chunk {chunk_index} timed out \
                     after {}s (retry also timed out)",
                    timeout.as_secs()
                );
                return Err(format!("Timed out after {}s (retried once)", timeout.as_secs()));
            }
        }
    }
    unreachable!("loop always returns within MAX_ATTEMPTS iterations")
}

/// A single provider round trip, racing the timeout against `cancel`. The request is rebuilt
/// fresh on every call (not passed in) because `LanguageModelRequest::generate_text` consumes
/// the request — a retry needs an entirely new builder chain, not a re-called future.
async fn run_chunk_attempt(
    provider: OpenAICompatible<DynamicModel>,
    system: String,
    prompt: String,
    timeout: Duration,
    chunk_index: usize,
    generation_id: &str,
    cancel: &CancellationToken,
) -> AttemptOutcome {
    let attempt = async {
        let mut req = LanguageModelRequest::builder()
            .model(provider)
            .system(system)
            .prompt(prompt)
            .temperature(0u32)
            .build();
        req.generate_text().await
    };

    let raced = tokio::select! {
        biased;
        _ = cancel.cancelled() => return AttemptOutcome::Cancelled,
        res = tokio::time::timeout(timeout, attempt) => res,
    };

    match raced {
        Err(_) => AttemptOutcome::TimedOut,
        Ok(Err(e)) => {
            log_api_error(chunk_index, generation_id, &e);
            AttemptOutcome::Failed(friendly_error(e))
        }
        Ok(Ok(resp)) => {
            let text = match resp.text() {
                Some(t) => t,
                None => return AttemptOutcome::Failed("Model returned no text content".to_string()),
            };
            log::trace!(
                "ai::generate: generation_id={generation_id} chunk {chunk_index} response={text}"
            );
            match extract_results(&text) {
                Ok((items, parse_path)) => AttemptOutcome::Success(ParsedChunk { items, parse_path }),
                Err(e) => AttemptOutcome::Failed(e),
            }
        }
    }
}

/// Simulates one chunk of a provider round trip for the Dev menu's "Mock AI" toggle: optional
/// artificial latency, then either a simulated failure or a deterministic transform of each
/// file's stem. No network involved, so this runs through the same reconcile/logging path a
/// real chunk would (see `generate`), just without the HTTP request.
async fn run_mock_chunk(
    mock: &MockAiConfig,
    chunk: &[FileEntry],
    chunk_index: usize,
    generation_id: &str,
) -> Result<ParsedChunk, String> {
    if mock.latency_ms > 0 {
        tokio::time::sleep(Duration::from_millis(mock.latency_ms as u64)).await;
    }
    if mock.fail_rate > 0.0 && mock_roll() < mock.fail_rate {
        log::debug!(
            "ai::mock: generation_id={generation_id} chunk {chunk_index} simulating a provider failure"
        );
        return Err("Mock AI: simulated provider failure".to_string());
    }
    let items = chunk
        .iter()
        .map(|e| AiResultItem {
            id: e.id.clone(),
            new_name: apply_mock_transform(mock.transform, &e.stem),
        })
        .collect();
    Ok(ParsedChunk {
        items,
        parse_path: ParsePath::WrappedObject,
    })
}

/// A `rand`-free pseudo-random float in `[0, 1)`, seeded from the system clock. Fine for "fail
/// roughly X% of mocked chunks" in dev tooling — never used anywhere security-sensitive.
fn mock_roll() -> f32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    (nanos % 1_000_000) as f32 / 1_000_000.0
}

fn apply_mock_transform(transform: MockTransform, stem: &str) -> String {
    match transform {
        MockTransform::Suffix => format!("{stem}_mock"),
        MockTransform::Uppercase => stem.to_uppercase(),
        MockTransform::Lowercase => stem.to_lowercase(),
        MockTransform::Reverse => stem.chars().rev().collect(),
        MockTransform::Slugify => stem
            .chars()
            .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-"),
    }
}

fn log_api_error(chunk_index: usize, generation_id: &str, e: &aisdk::Error) {
    if let aisdk::Error::ApiError {
        status_code: Some(code),
        details,
        ..
    } = e
    {
        log::warn!(
            "ai::generate: generation_id={generation_id} chunk {chunk_index} API error status={} details={details}",
            code.as_u16(),
        );
    } else {
        log::warn!(
            "ai::generate: generation_id={generation_id} chunk {chunk_index} provider error: {e}"
        );
    }
}

struct ParsedChunk {
    items: Vec<AiResultItem>,
    parse_path: ParsePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParsePath {
    WrappedObject,
    BareArray,
    SubstringWrapped,
    SubstringArray,
}

impl ParsePath {
    fn as_str(self) -> &'static str {
        match self {
            Self::WrappedObject => "wrapped_object",
            Self::BareArray => "bare_array",
            Self::SubstringWrapped => "substring_wrapped",
            Self::SubstringArray => "substring_array",
        }
    }
}

struct ReconcileReport {
    items: Vec<AiResultItem>,
    model_count: usize,
    dropped_unknown: usize,
    sanitized_count: usize,
    missing_ids: Vec<String>,
}

fn log_chunk_reconcile(
    chunk_index: usize,
    generation_id: &str,
    report: &ReconcileReport,
    stems: &HashMap<String, String>,
    parse_path: ParsePath,
    elapsed_ms: u128,
) {
    log::debug!(
        "ai::generate: generation_id={generation_id} chunk {chunk_index} ok in {elapsed_ms}ms — \
         parse={}, model_returned={}, kept={}, dropped_unknown={}, sanitized={}, missing={}",
        parse_path.as_str(),
        report.model_count,
        report.items.len(),
        report.dropped_unknown,
        report.sanitized_count,
        report.missing_ids.len()
    );
    if !report.missing_ids.is_empty() {
        log::debug!(
            "ai::generate: generation_id={generation_id} chunk {chunk_index} missing ids \
             (engine will keep original name): {:?}",
            report.missing_ids
        );
    }
    for item in &report.items {
        let old = stems.get(&item.id).map(String::as_str).unwrap_or("?");
        if old != item.new_name {
            log::debug!(
                "ai::generate: generation_id={generation_id} chunk {chunk_index} rename id={} \"{old}\" -> \"{}\"",
                item.id,
                item.new_name
            );
        } else {
            log::trace!(
                "ai::generate: generation_id={generation_id} chunk {chunk_index} unchanged id={} \"{old}\"",
                item.id
            );
        }
    }
}

fn trunc_log(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}… ({} chars total)", &s[..max], s.len())
    }
}

fn friendly_error(e: aisdk::Error) -> String {
    if let aisdk::Error::ApiError {
        status_code: Some(code),
        ..
    } = &e
    {
        if code.as_u16() == 401 || code.as_u16() == 403 {
            return "Provider rejected the API key".to_string();
        }
    }
    e.to_string()
}

#[derive(Deserialize)]
struct Wrapped {
    results: Vec<AiResultItem>,
}

/// Lenient JSON extraction: try `{"results":[...]}`, then a bare array, then fall back to
/// the substring between the first `{`/`[` and the last `}`/`]` (handles ```json fences and
/// prose wrapping the model may add despite instructions).
fn extract_results(text: &str) -> Result<(Vec<AiResultItem>, ParsePath), String> {
    let trimmed = text.trim();

    if let Ok(w) = serde_json::from_str::<Wrapped>(trimmed) {
        return Ok((w.results, ParsePath::WrappedObject));
    }
    if let Ok(items) = serde_json::from_str::<Vec<AiResultItem>>(trimmed) {
        return Ok((items, ParsePath::BareArray));
    }

    let start = trimmed.find(['{', '[']);
    let end = trimmed.rfind(['}', ']']);
    if let (Some(s), Some(e)) = (start, end) {
        if e > s {
            let sub = &trimmed[s..=e];
            if let Ok(w) = serde_json::from_str::<Wrapped>(sub) {
                return Ok((w.results, ParsePath::SubstringWrapped));
            }
            if let Ok(items) = serde_json::from_str::<Vec<AiResultItem>>(sub) {
                return Ok((items, ParsePath::SubstringArray));
            }
        }
    }

    let preview = trunc_log(trimmed, 500);
    log::warn!("ai::extract_results: JSON parse failed — response preview: {preview}");
    Err(format!("Could not parse model response as JSON: {preview}"))
}

/// Drop hallucinated ids and sanitize each `newName` (no path separators, no leaked
/// extension). Duplicate ids are left as-is; the engine's `HashMap` collect keeps the last
/// occurrence, so order here is preserved from the model's own (chunk-local) response order.
fn reconcile(
    items: Vec<AiResultItem>,
    ids: &HashSet<String>,
    stems: &HashMap<String, String>,
    exts: &HashMap<String, String>,
    generation_id: &str,
) -> ReconcileReport {
    let model_count = items.len();
    let mut dropped_unknown = 0usize;
    let mut sanitized_count = 0usize;
    let mut kept_ids = HashSet::new();
    let mut out = Vec::new();

    for item in items {
        if !ids.contains(&item.id) {
            dropped_unknown += 1;
            log::debug!(
                "ai::reconcile: generation_id={generation_id} dropped unknown id={} newName={:?}",
                item.id,
                item.new_name
            );
            continue;
        }
        let ext = exts.get(&item.id).map(String::as_str).unwrap_or("");
        let raw = item.new_name.clone();
        let new_name = sanitize_name(&raw, ext);
        if new_name != raw.trim() {
            sanitized_count += 1;
            log::debug!(
                "ai::reconcile: generation_id={generation_id} sanitized id={}: {:?} -> {:?}",
                item.id,
                raw,
                new_name
            );
        }
        kept_ids.insert(item.id.clone());
        out.push(AiResultItem {
            id: item.id,
            new_name,
        });
    }

    let missing_ids: Vec<String> = ids
        .iter()
        .filter(|id| !kept_ids.contains(*id))
        .cloned()
        .collect();

    if dropped_unknown > 0 {
        log::debug!(
            "ai::reconcile: generation_id={generation_id} dropped {dropped_unknown} hallucinated id(s) (of {model_count} returned)"
        );
    }
    if !missing_ids.is_empty() {
        let missing_preview: Vec<_> = missing_ids
            .iter()
            .take(10)
            .map(|id| format!("{id}({})", stems.get(id).map(String::as_str).unwrap_or("?")))
            .collect();
        let suffix = if missing_ids.len() > 10 {
            format!(" … +{} more", missing_ids.len() - 10)
        } else {
            String::new()
        };
        log::debug!(
            "ai::reconcile: generation_id={generation_id} {}/{} input file(s) missing from model response: {missing_preview:?}{suffix}",
            missing_ids.len(),
            ids.len()
        );
    }

    ReconcileReport {
        items: out,
        model_count,
        dropped_unknown,
        sanitized_count,
        missing_ids,
    }
}

fn sanitize_name(name: &str, ext: &str) -> String {
    let stripped: String = name.chars().filter(|c| *c != '/' && *c != '\\').collect();
    let trimmed = stripped.trim();
    if !ext.is_empty() {
        if let Some(stem) = trimmed.strip_suffix(&format!(".{ext}")) {
            return stem.to_string();
        }
    }
    trimmed.to_string()
}

fn system_prompt(max_len: u32) -> String {
    format!(
        "You rename files. Given a JSON array of files (each `id`, current `name` without \
         extension, `ext`, `parentHint`) and an instruction, return ONLY \
         {{\"results\":[{{\"id\":\"...\",\"newName\":\"...\"}}]}}. Keep each `id` exactly; one \
         result per file; `newName` is the stem only — never an extension, path, or separator; \
         keep under {max_len} chars; don't invent ids. Use your general knowledge to follow the \
         instruction — e.g. adding a known author, date, or topic — even if that information \
         isn't present in the filename itself. Only echo the original name when you cannot \
         confidently improve it at all."
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PromptFile<'a> {
    id: &'a str,
    name: &'a str,
    ext: &'a str,
    parent_hint: &'a str,
    index: usize,
}

fn user_prompt(prompt: &str, chunk: &[FileEntry], offset: usize) -> String {
    let files: Vec<PromptFile> = chunk
        .iter()
        .enumerate()
        .map(|(i, e)| PromptFile {
            id: &e.id,
            name: &e.stem,
            ext: &e.ext,
            parent_hint: &e.parent_dir,
            index: offset + i,
        })
        .collect();
    let json = serde_json::to_string(&files).unwrap_or_default();
    format!("Instruction: {prompt}\n\nFiles: {json}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn entry(id: &str, stem: &str, ext: &str) -> FileEntry {
        FileEntry {
            id: id.to_string(),
            path: format!("/tmp/{id}"),
            parent_dir: "/tmp".to_string(),
            stem: stem.to_string(),
            ext: ext.to_string(),
            is_dir: false,
            size: 0,
            modified: None,
        }
    }

    // ---- extract_results -----------------------------------------------------------

    #[test]
    fn extract_results_clean_object() {
        let (got, path) = extract_results(r#"{"results":[{"id":"a","newName":"A"}]}"#).unwrap();
        assert_eq!(path, ParsePath::WrappedObject);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id, "a");
        assert_eq!(got[0].new_name, "A");
    }

    #[test]
    fn extract_results_bare_array() {
        let (got, path) = extract_results(r#"[{"id":"a","newName":"A"}]"#).unwrap();
        assert_eq!(path, ParsePath::BareArray);
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn extract_results_fenced() {
        let (got, _) = extract_results("```json\n{\"results\":[{\"id\":\"a\",\"newName\":\"A\"}]}\n```")
            .unwrap();
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn extract_results_prose_wrapped() {
        let (got, _) = extract_results(
            "Sure, here you go:\n{\"results\":[{\"id\":\"a\",\"newName\":\"A\"}]}\nHope that helps!",
        )
        .unwrap();
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn extract_results_garbage_errs() {
        assert!(extract_results("not json at all").is_err());
    }

    // ---- chunk_entries ---------------------------------------------------------------

    #[test]
    fn chunk_entries_splits_evenly() {
        let entries: Vec<FileEntry> = (0..95).map(|i| entry(&i.to_string(), "f", "txt")).collect();
        let chunks = chunk_entries(entries, 40);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 40);
        assert_eq!(chunks[1].len(), 40);
        assert_eq!(chunks[2].len(), 15);
    }

    // ---- reconcile ---------------------------------------------------------------

    #[test]
    fn reconcile_drops_unknown_ids_and_strips_extension() {
        let ids: HashSet<String> = ["a".to_string()].into_iter().collect();
        let stems: HashMap<String, String> = [("a".to_string(), "old".to_string())].into();
        let exts: HashMap<String, String> = [("a".to_string(), "txt".to_string())].into();
        let items = vec![
            AiResultItem {
                id: "a".to_string(),
                new_name: "New Name.txt".to_string(),
            },
            AiResultItem {
                id: "unknown".to_string(),
                new_name: "Ghost".to_string(),
            },
        ];
        let report = reconcile(items, &ids, &stems, &exts, "test-gen");
        assert_eq!(report.items.len(), 1);
        assert_eq!(report.items[0].new_name, "New Name");
        assert_eq!(report.dropped_unknown, 1);
        assert_eq!(report.sanitized_count, 1);
    }

    #[test]
    fn sanitize_strips_path_separators() {
        assert_eq!(sanitize_name("a/b\\c.txt", "txt"), "abc");
    }

    // ---- HTTP-level tests against a mocked /chat/completions endpoint ----------------

    fn chat_response(content: &str) -> serde_json::Value {
        serde_json::json!({
            "id": "test",
            "object": "chat.completion",
            "created": 0,
            "model": "test-model",
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": content },
                "finish_reason": "stop"
            }]
        })
    }

    fn test_profile(base_url: String) -> ProviderProfile {
        ProviderProfile {
            id: "p1".to_string(),
            label: "test".to_string(),
            base_url,
            model: "test-model".to_string(),
            chunk_size: 40,
            concurrency: 3,
            max_len: 80,
            timeout_secs: 60,
            has_key: false,
        }
    }

    #[tokio::test]
    async fn generate_happy_path() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(chat_response(
                r#"{"results":[{"id":"a","newName":"Alpha"}]}"#,
            )))
            .mount(&server)
            .await;

        let cfg = test_profile(server.uri());
        let report = generate(
            &cfg,
            "sk-test",
            "rename".to_string(),
            vec![entry("a", "old", "txt")],
            "test-gen",
            Arc::new(NoopProgressEmitter),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();

        assert_eq!(report.failed_chunks, 0);
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].new_name, "Alpha");
    }

    #[tokio::test]
    async fn generate_partial_failure_on_500() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let mut cfg = test_profile(server.uri());
        cfg.chunk_size = 1;
        let entries = vec![entry("a", "old", "txt")];
        let err = generate(
            &cfg,
            "sk-test",
            "rename".to_string(),
            entries,
            "test-gen",
            Arc::new(NoopProgressEmitter),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert!(!err.is_empty());
    }

    #[tokio::test]
    async fn generate_mixed_success_and_failure_reports_warning() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(chat_response(r#"{"results":[{"id":"a","newName":"Alpha"}]}"#)),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let mut cfg = test_profile(server.uri());
        cfg.chunk_size = 1;
        cfg.concurrency = 1;
        let entries = vec![entry("a", "old-a", "txt"), entry("b", "old-b", "txt")];
        let report = generate(
            &cfg,
            "sk-test",
            "rename".to_string(),
            entries,
            "test-gen",
            Arc::new(NoopProgressEmitter),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();

        assert_eq!(report.total_chunks, 2);
        assert_eq!(report.failed_chunks, 1);
        assert_eq!(report.results.len(), 1);
        assert!(report.warning.is_some());
    }

    #[tokio::test]
    async fn generate_timeout_counts_as_chunk_failure() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(chat_response(r#"{"results":[{"id":"a","newName":"Alpha"}]}"#))
                    .set_delay(Duration::from_secs(2)),
            )
            .mount(&server)
            .await;

        let mut cfg = test_profile(server.uri());
        cfg.timeout_secs = 1; // `generate` clamps to a 1s minimum, so this is the fastest case
        let entries = vec![entry("a", "old", "txt")];
        let err = generate(
            &cfg,
            "sk-test",
            "rename".to_string(),
            entries,
            "test-gen",
            Arc::new(NoopProgressEmitter),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert!(err.contains("Timed out"));
    }

    #[tokio::test]
    async fn generate_retries_once_after_timeout_then_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(chat_response(r#"{"results":[{"id":"a","newName":"Alpha"}]}"#))
                    .set_delay(Duration::from_secs(3)),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(chat_response(r#"{"results":[{"id":"a","newName":"Alpha"}]}"#)),
            )
            .mount(&server)
            .await;

        let mut cfg = test_profile(server.uri());
        cfg.timeout_secs = 1; // first attempt (3s delay) times out, retry (no delay) succeeds
        let entries = vec![entry("a", "old", "txt")];
        let report = generate(
            &cfg,
            "sk-test",
            "rename".to_string(),
            entries,
            "test-gen",
            Arc::new(NoopProgressEmitter),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();

        assert_eq!(report.failed_chunks, 0);
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].new_name, "Alpha");
    }

    #[tokio::test]
    async fn generate_gives_up_after_one_retry_on_repeated_timeout() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(chat_response(r#"{"results":[{"id":"a","newName":"Alpha"}]}"#))
                    .set_delay(Duration::from_secs(3)),
            )
            .expect(2) // exactly one attempt + one retry, never more
            .mount(&server)
            .await;

        let mut cfg = test_profile(server.uri());
        cfg.timeout_secs = 1;
        let entries = vec![entry("a", "old", "txt")];
        let err = generate(
            &cfg,
            "sk-test",
            "rename".to_string(),
            entries,
            "test-gen",
            Arc::new(NoopProgressEmitter),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert!(err.contains("Timed out"));
        assert!(err.contains("retried"));
        // `.expect(2)` above is verified automatically by wiremock when `server` drops.
    }

    #[tokio::test]
    async fn generate_does_not_retry_non_timeout_failures() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .expect(1) // no retry for a plain API error
            .mount(&server)
            .await;

        let mut cfg = test_profile(server.uri());
        cfg.chunk_size = 1;
        let entries = vec![entry("a", "old", "txt")];
        let err = generate(
            &cfg,
            "sk-test",
            "rename".to_string(),
            entries,
            "test-gen",
            Arc::new(NoopProgressEmitter),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert!(!err.is_empty());
    }

    #[tokio::test]
    async fn generate_cancelled_mid_flight_reports_cancelled_not_timeout() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(chat_response(r#"{"results":[{"id":"a","newName":"Alpha"}]}"#))
                    .set_delay(Duration::from_secs(5)),
            )
            .mount(&server)
            .await;

        let mut cfg = test_profile(server.uri());
        cfg.timeout_secs = 30; // long enough that cancellation, not the timeout, wins the race
        let entries = vec![entry("a", "old", "txt")];
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            cancel_clone.cancel();
        });

        let err = generate(
            &cfg,
            "sk-test",
            "rename".to_string(),
            entries,
            "test-gen",
            Arc::new(NoopProgressEmitter),
            None,
            cancel,
        )
        .await
        .unwrap_err();
        assert!(err.contains("Cancelled"), "expected Cancelled, got: {err}");
    }

    #[tokio::test]
    async fn generate_cancelled_before_dispatch_short_circuits() {
        let cfg = test_profile("http://127.0.0.1:1".to_string());
        let entries = vec![entry("a", "old", "txt")];
        let cancel = CancellationToken::new();
        cancel.cancel(); // already cancelled before `generate` even starts chunking

        let err = generate(
            &cfg,
            "sk-test",
            "rename".to_string(),
            entries,
            "test-gen",
            Arc::new(NoopProgressEmitter),
            None,
            cancel,
        )
        .await
        .unwrap_err();
        assert_eq!(err, "Cancelled");
    }

    // ---- Mock AI (Dev menu) -----------------------------------------------------------

    #[tokio::test]
    async fn mock_generate_applies_transform_without_hitting_network() {
        // An unroutable base_url: if mocking somehow fell through to a real request, this
        // would fail/hang instead of returning the expected mocked result.
        let cfg = test_profile("http://127.0.0.1:1".to_string());
        let mock = MockAiConfig {
            enabled: true,
            latency_ms: 0,
            fail_rate: 0.0,
            transform: MockTransform::Uppercase,
        };
        let report = generate(
            &cfg,
            "",
            "rename".to_string(),
            vec![entry("a", "old", "txt")],
            "test-gen",
            Arc::new(NoopProgressEmitter),
            Some(mock),
            CancellationToken::new(),
        )
        .await
        .unwrap();

        assert_eq!(report.failed_chunks, 0);
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].new_name, "OLD");
    }

    #[tokio::test]
    async fn mock_generate_fail_rate_one_always_fails() {
        let cfg = test_profile("http://127.0.0.1:1".to_string());
        let mock = MockAiConfig {
            enabled: true,
            latency_ms: 0,
            fail_rate: 1.0,
            transform: MockTransform::Suffix,
        };
        let err = generate(
            &cfg,
            "",
            "rename".to_string(),
            vec![entry("a", "old", "txt")],
            "test-gen",
            Arc::new(NoopProgressEmitter),
            Some(mock),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert!(err.contains("simulated"));
    }

    #[test]
    fn apply_mock_transform_variants() {
        assert_eq!(apply_mock_transform(MockTransform::Suffix, "photo"), "photo_mock");
        assert_eq!(apply_mock_transform(MockTransform::Uppercase, "photo"), "PHOTO");
        assert_eq!(apply_mock_transform(MockTransform::Lowercase, "PHOTO"), "photo");
        assert_eq!(apply_mock_transform(MockTransform::Reverse, "abc"), "cba");
        assert_eq!(apply_mock_transform(MockTransform::Slugify, "My Photo!!"), "my-photo");
    }
}
