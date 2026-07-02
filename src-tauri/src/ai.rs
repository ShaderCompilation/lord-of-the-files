//! BYOK AI rename: dispatches chunks of files to a user-configured OpenAI-compatible
//! endpoint via the `aisdk` crate, and lenient-parses the JSON the model returns.
//!
//! No `response_format`/structured-output is ever requested, so providers/models that would
//! hard-reject that field still work — we rely entirely on `extract_results` to recover JSON
//! from a plain text response (fenced in ```json, wrapped in prose, or clean).

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use aisdk::core::{DynamicModel, LanguageModelRequest};
use aisdk::providers::OpenAICompatible;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::settings::{MockAiConfig, MockTransform, ProviderProfile};
use crate::types::{AiChunkDetail, AiGenerateReport, AiRequestMeta, AiResultItem, FileEntry};

/// Placeholder sent when a profile has no key configured (e.g. local Ollama / LM Studio),
/// since `aisdk`'s `OpenAICompatible` builder rejects an empty `api_key` even though these
/// local servers never check the `Authorization` header.
const NO_KEY_PLACEHOLDER: &str = "not-needed";

#[allow(clippy::too_many_arguments)]
pub async fn generate(
    cfg: &ProviderProfile,
    api_key: &str,
    prompt: String,
    entries: Vec<FileEntry>,
    generation_id: &str,
    mock: Option<MockAiConfig>,
    cancel: CancellationToken,
) -> AiGenerateReport {
    // Belt-and-suspenders: a release binary never mocks, even if `enabled` was left on in
    // settings.db by a previous dev build, or a command were invoked directly.
    let mock = if cfg!(debug_assertions) { mock } else { None };

    let chunk_size = (cfg.chunk_size as usize).max(1);
    let concurrency = (cfg.concurrency as usize).max(1);
    let timeout = Duration::from_secs(cfg.timeout_secs.max(1) as u64);
    let system = system_prompt(cfg.max_len);
    let entry_count = entries.len();
    let has_key = !api_key.is_empty();

    let request = AiRequestMeta {
        generation_id: generation_id.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        profile_id: cfg.id.clone(),
        profile_label: cfg.label.clone(),
        base_url: cfg.base_url.clone(),
        model: cfg.model.clone(),
        instruction: prompt.clone(),
        system_prompt: system.clone(),
        entry_count,
        chunk_size: chunk_size as u32,
        concurrency: concurrency as u32,
        timeout_secs: cfg.timeout_secs,
        max_len: cfg.max_len,
        temperature: 0.0,
        mock: mock.is_some(),
        has_key,
    };

    if cancel.is_cancelled() {
        log::debug!("ai::generate: generation_id={generation_id} cancelled before dispatch");
        return empty_report(request, "Cancelled".to_string());
    }

    let key = if api_key.is_empty() {
        NO_KEY_PLACEHOLDER
    } else {
        api_key
    };
    // Skip building a real client entirely when mocking — `cfg.base_url`/`model` don't need to
    // be valid (or even set) for a mocked generation.
    let provider = if mock.is_none() {
        match OpenAICompatible::<DynamicModel>::builder()
            .base_url(&cfg.base_url)
            .api_key(key)
            .model_name(&cfg.model)
            .build()
        {
            Ok(p) => Some(p),
            Err(e) => return empty_report(request, e.to_string()),
        }
    } else {
        None
    };

    let chunks = chunk_entries(entries, chunk_size);
    let total_chunks = chunks.len();
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
        let file_count = chunk.len();
        let ids: HashSet<String> = chunk.iter().map(|e| e.id.clone()).collect();
        let stems: HashMap<String, String> =
            chunk.iter().map(|e| (e.id.clone(), e.stem.clone())).collect();
        let exts: HashMap<String, String> =
            chunk.iter().map(|e| (e.id.clone(), e.ext.clone())).collect();
        let gen_id = generation_id.to_string();
        let cancel = cancel.clone();
        async move {
            if cancel.is_cancelled() {
                log::debug!(
                    "ai::generate: generation_id={gen_id} chunk {chunk_index} skipped (already cancelled)"
                );
                let detail = AiChunkDetail {
                    chunk_index,
                    file_count,
                    user_prompt: user,
                    raw_response: None,
                    error: Some("Cancelled".to_string()),
                    parse_path: None,
                    elapsed_ms: 0,
                    model_count: None,
                    dropped_unknown: None,
                    sanitized_count: None,
                    missing_ids: Vec::new(),
                };
                return (chunk_index, Err("Cancelled".to_string()), detail);
            }

            log::debug!(
                "ai::generate: generation_id={gen_id} chunk {chunk_index}/{total_chunks} dispatching {file_count} file(s)"
            );
            log::trace!(
                "ai::generate: generation_id={gen_id} chunk {chunk_index} user_prompt={user}"
            );

            let started = Instant::now();
            let outcome = match &mock {
                Some(mock_cfg) => run_mock_chunk(mock_cfg, &chunk, chunk_index, &gen_id).await,
                None => match provider {
                    Some(provider) => {
                        run_chunk(
                            provider,
                            system,
                            user.clone(),
                            timeout,
                            chunk_index,
                            &gen_id,
                            &cancel,
                        )
                        .await
                    }
                    None => Err(ChunkError {
                        message: "Internal error: no AI provider configured (mocking was off but \
                                   no client was built) — this is a bug, not a network/provider \
                                   failure"
                            .to_string(),
                        raw_response: None,
                    }),
                },
            };
            let elapsed_ms = started.elapsed().as_millis() as u64;

            let (result, detail) = match outcome {
                Ok(parsed) => {
                    let report = reconcile(parsed.items, &ids, &stems, &exts, &gen_id);
                    log_chunk_reconcile(
                        chunk_index,
                        &gen_id,
                        &report,
                        &stems,
                        parsed.parse_path,
                        elapsed_ms as u128,
                    );
                    let detail = AiChunkDetail {
                        chunk_index,
                        file_count,
                        user_prompt: user,
                        raw_response: Some(parsed.raw_text),
                        error: None,
                        parse_path: Some(parsed.parse_path.as_str().to_string()),
                        elapsed_ms,
                        model_count: Some(report.model_count),
                        dropped_unknown: Some(report.dropped_unknown),
                        sanitized_count: Some(report.sanitized_count),
                        missing_ids: report.missing_ids.clone(),
                    };
                    (Ok(report.items), detail)
                }
                Err(e) => {
                    log::warn!(
                        "ai::generate: generation_id={gen_id} chunk {chunk_index} failed after {elapsed_ms}ms: {}",
                        e.message
                    );
                    let detail = AiChunkDetail {
                        chunk_index,
                        file_count,
                        user_prompt: user,
                        raw_response: e.raw_response,
                        error: Some(e.message.clone()),
                        parse_path: None,
                        elapsed_ms,
                        model_count: None,
                        dropped_unknown: None,
                        sanitized_count: None,
                        missing_ids: Vec::new(),
                    };
                    (Err(e.message), detail)
                }
            };

            (chunk_index, result, detail)
        }
    });

    let mut by_chunk: Vec<(usize, Result<Vec<AiResultItem>, String>, AiChunkDetail)> =
        stream::iter(tasks).buffer_unordered(concurrency).collect().await;
    by_chunk.sort_by_key(|(i, _, _)| *i);

    let mut results = Vec::new();
    let mut failed_chunks = 0u32;
    let mut first_error: Option<String> = None;
    let mut chunk_details = Vec::with_capacity(by_chunk.len());
    for (_, outcome, detail) in by_chunk {
        match outcome {
            Ok(items) => results.extend(items),
            Err(e) => {
                failed_chunks += 1;
                first_error.get_or_insert(e);
            }
        }
        chunk_details.push(detail);
    }

    if total_chunks > 0 && failed_chunks as usize == total_chunks {
        let err = first_error.unwrap_or_else(|| "AI generation failed".to_string());
        log::warn!("ai::generate: generation_id={generation_id} all chunks failed: {err}");
        return AiGenerateReport {
            results,
            failed_chunks,
            total_chunks: total_chunks as u32,
            warning: None,
            error: Some(err),
            request,
            chunks: chunk_details,
        };
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

    AiGenerateReport {
        results,
        failed_chunks,
        total_chunks: total_chunks as u32,
        warning,
        error: None,
        request,
        chunks: chunk_details,
    }
}

/// Builds a report for a generation that never dispatched any chunk at all (cancelled before
/// dispatch, or the provider client failed to build) — still carries full request metadata so
/// the attempt stays inspectable in AI History, just with no chunk detail to show.
fn empty_report(request: AiRequestMeta, error: String) -> AiGenerateReport {
    AiGenerateReport {
        results: Vec::new(),
        failed_chunks: 0,
        total_chunks: 0,
        warning: None,
        error: Some(error),
        request,
        chunks: Vec::new(),
    }
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

/// A terminal chunk failure, carrying whatever response text was actually received (if any) so
/// it can still be shown in the "Details" dialog / AI History even when parsing or reconciling
/// failed — `raw_response` is `None` only when no response body was ever obtained (network
/// error, timeout, or cancellation before a response arrived).
struct ChunkError {
    message: String,
    raw_response: Option<String>,
}

/// Outcome of a single attempt inside `run_chunk`, distinguishing "timed out" (retryable) from
/// "cancelled" (never retried) and other terminal failures, so the retry loop can decide
/// correctly without string-matching error messages.
enum AttemptOutcome {
    Success(ParsedChunk),
    TimedOut,
    Cancelled,
    Failed(ChunkError),
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
) -> Result<ParsedChunk, ChunkError> {
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
                return Err(ChunkError {
                    message: "Cancelled".to_string(),
                    raw_response: None,
                });
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
                return Err(ChunkError {
                    message: format!("Timed out after {}s (retried once)", timeout.as_secs()),
                    raw_response: None,
                });
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
            AttemptOutcome::Failed(ChunkError {
                message: friendly_error(e),
                raw_response: None,
            })
        }
        Ok(Ok(resp)) => {
            let text = match resp.text() {
                Some(t) => t,
                None => {
                    return AttemptOutcome::Failed(ChunkError {
                        message: "Model returned no text content".to_string(),
                        raw_response: None,
                    })
                }
            };
            log::trace!(
                "ai::generate: generation_id={generation_id} chunk {chunk_index} response={text}"
            );
            match extract_results(&text) {
                Ok((items, parse_path)) => AttemptOutcome::Success(ParsedChunk {
                    items,
                    parse_path,
                    raw_text: text,
                }),
                Err(e) => AttemptOutcome::Failed(ChunkError {
                    message: e,
                    raw_response: Some(text),
                }),
            }
        }
    }
}

/// Simulates one chunk of a provider round trip for the Dev menu's "Mock AI" toggle: optional
/// artificial latency, then either a simulated failure or a deterministic transform of each
/// file's stem. No network involved, so this runs through the same reconcile/logging path a
/// real chunk would (see `generate`), just without the HTTP request. `raw_response` on success
/// is a synthesized rendering of the mocked items (there's no real wire text to show).
async fn run_mock_chunk(
    mock: &MockAiConfig,
    chunk: &[FileEntry],
    chunk_index: usize,
    generation_id: &str,
) -> Result<ParsedChunk, ChunkError> {
    if mock.latency_ms > 0 {
        tokio::time::sleep(Duration::from_millis(mock.latency_ms as u64)).await;
    }
    if mock.fail_rate > 0.0 && mock_roll() < mock.fail_rate {
        log::debug!(
            "ai::mock: generation_id={generation_id} chunk {chunk_index} simulating a provider failure"
        );
        return Err(ChunkError {
            message: "Mock AI: simulated provider failure".to_string(),
            raw_response: None,
        });
    }
    let items: Vec<AiResultItem> = chunk
        .iter()
        .map(|e| AiResultItem {
            id: e.id.clone(),
            new_name: apply_mock_transform(mock.transform, &e.stem),
        })
        .collect();
    let raw_text =
        serde_json::to_string(&serde_json::json!({ "results": items })).unwrap_or_default();
    Ok(ParsedChunk {
        items,
        parse_path: ParsePath::WrappedObject,
        raw_text,
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
    raw_text: String,
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
    // Drop path separators (unchanged) and other OS-invalid chars; map ':' to " -"
    // (near-universal title/subtitle separator in LLM output); drop control chars.
    let mut filtered = String::with_capacity(name.len());
    for c in name.chars() {
        match c {
            '/' | '\\' | '<' | '>' | '"' | '|' | '?' | '*' => {}
            ':' => filtered.push_str(" -"),
            c if (c as u32) < 0x20 => {}
            c => filtered.push(c),
        }
    }

    // Collapse whitespace runs the substitutions above may have introduced, and trim.
    let collapsed = crate::engine::steps::clean_up(&filtered, true, true, None, false);

    // Strip a trailing dot (validate_name also rejects names ending in '.').
    let trimmed_dot = collapsed.trim_end_matches('.');

    // Strip a leaked "<stem>.<ext>" suffix on the now-trimmed string.
    let stem = if ext.is_empty() {
        trimmed_dot
    } else {
        trimmed_dot.strip_suffix(&format!(".{ext}")).unwrap_or(trimmed_dot)
    };

    // Extension-stripping can re-expose a trailing '.' or space — trim once more.
    stem.trim().trim_end_matches('.').trim().to_string()
}

fn system_prompt(max_len: u32) -> String {
    format!(
        "You rename files. Given a JSON array of files (each `id`, current `name` without \
         extension, `ext`, `parentHint`) and an instruction, return ONLY \
         {{\"results\":[{{\"id\":\"...\",\"newName\":\"...\"}}]}}. Keep each `id` exactly; one \
         result per file; `newName` is the stem only — never an extension, path, or separator; \
         use only characters valid in filenames — no `: < > \" / \\ | ? *` or control \
         characters — and write subtitles as \" - \" instead of a colon, e.g. \"Title - \
         Subtitle\" not \"Title: Subtitle\"; keep under {max_len} chars; don't invent ids. Use \
         your general knowledge to follow the instruction — e.g. adding a known author, date, \
         or topic — even if that information isn't present in the filename itself, but don't \
         invent an author, date, or detail you're not reasonably confident about. `parentHint` \
         is the enclosing folder name — useful context (e.g. series, author, or subject) but \
         never to be copied verbatim into `newName` unless it genuinely belongs there. Only \
         echo the original name when you cannot confidently improve it at all."
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

    #[test]
    fn sanitize_replaces_colon_with_dash() {
        assert_eq!(
            sanitize_name("Windows Security Internals: A Deep Dive - James Forshaw", ""),
            "Windows Security Internals - A Deep Dive - James Forshaw"
        );
    }

    #[test]
    fn sanitize_strips_other_invalid_chars() {
        assert_eq!(sanitize_name("Report <v2> \"final\"?*|", ""), "Report v2 final");
    }

    #[test]
    fn sanitize_strips_control_chars() {
        assert_eq!(sanitize_name("Bad\u{0007}Name\u{0001}Here", ""), "BadNameHere");
    }

    #[test]
    fn sanitize_strips_trailing_dot() {
        assert_eq!(sanitize_name("Trailing Dot.", ""), "Trailing Dot");
    }

    #[test]
    fn sanitize_still_strips_leaked_extension() {
        assert_eq!(sanitize_name("New Name.txt", "txt"), "New Name");
    }

    #[test]
    fn sanitize_handles_leaked_extension_exposing_trailing_dot() {
        // "Foo..txt" with ext "txt": strip ".txt" suffix -> "Foo." -> must re-trim trailing dot.
        assert_eq!(sanitize_name("Foo..txt", "txt"), "Foo");
    }

    #[test]
    fn sanitize_trims_extra_whitespace_around_colon() {
        assert_eq!(sanitize_name("Title  :   Subtitle", ""), "Title - Subtitle");
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
            max_len: 300,
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
            None,
            CancellationToken::new(),
        )
        .await;

        assert!(report.error.is_none());
        assert_eq!(report.failed_chunks, 0);
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].new_name, "Alpha");
        assert_eq!(report.chunks.len(), 1);
        assert_eq!(report.chunks[0].raw_response.as_deref(), Some(r#"{"results":[{"id":"a","newName":"Alpha"}]}"#));
        assert_eq!(report.request.model, "test-model");
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
        let report = generate(
            &cfg,
            "sk-test",
            "rename".to_string(),
            entries,
            "test-gen",
            None,
            CancellationToken::new(),
        )
        .await;
        let err = report.error.expect("expected a hard failure");
        assert!(!err.is_empty());
        assert_eq!(report.chunks.len(), 1);
        assert!(report.chunks[0].error.is_some());
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
            None,
            CancellationToken::new(),
        )
        .await;

        assert!(report.error.is_none());
        assert_eq!(report.total_chunks, 2);
        assert_eq!(report.failed_chunks, 1);
        assert_eq!(report.results.len(), 1);
        assert!(report.warning.is_some());
        assert_eq!(report.chunks.len(), 2);
        assert_eq!(report.chunks.iter().filter(|c| c.error.is_some()).count(), 1);
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
        let report = generate(
            &cfg,
            "sk-test",
            "rename".to_string(),
            entries,
            "test-gen",
            None,
            CancellationToken::new(),
        )
        .await;
        let err = report.error.expect("expected a hard failure");
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
            None,
            CancellationToken::new(),
        )
        .await;

        assert!(report.error.is_none());
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
        let report = generate(
            &cfg,
            "sk-test",
            "rename".to_string(),
            entries,
            "test-gen",
            None,
            CancellationToken::new(),
        )
        .await;
        let err = report.error.expect("expected a hard failure");
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
        let report = generate(
            &cfg,
            "sk-test",
            "rename".to_string(),
            entries,
            "test-gen",
            None,
            CancellationToken::new(),
        )
        .await;
        let err = report.error.expect("expected a hard failure");
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

        let report = generate(
            &cfg,
            "sk-test",
            "rename".to_string(),
            entries,
            "test-gen",
            None,
            cancel,
        )
        .await;
        let err = report.error.expect("expected a hard failure");
        assert!(err.contains("Cancelled"), "expected Cancelled, got: {err}");
    }

    #[tokio::test]
    async fn generate_cancelled_before_dispatch_short_circuits() {
        let cfg = test_profile("http://127.0.0.1:1".to_string());
        let entries = vec![entry("a", "old", "txt")];
        let cancel = CancellationToken::new();
        cancel.cancel(); // already cancelled before `generate` even starts chunking

        let report = generate(
            &cfg,
            "sk-test",
            "rename".to_string(),
            entries,
            "test-gen",
            None,
            cancel,
        )
        .await;
        assert_eq!(report.error.as_deref(), Some("Cancelled"));
        assert!(report.chunks.is_empty());
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
            Some(mock),
            CancellationToken::new(),
        )
        .await;

        assert!(report.error.is_none());
        assert_eq!(report.failed_chunks, 0);
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].new_name, "OLD");
        assert!(report.request.mock);
        assert!(report.chunks[0].raw_response.is_some());
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
        let report = generate(
            &cfg,
            "",
            "rename".to_string(),
            vec![entry("a", "old", "txt")],
            "test-gen",
            Some(mock),
            CancellationToken::new(),
        )
        .await;
        let err = report.error.expect("expected a hard failure");
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
