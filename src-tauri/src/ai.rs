//! BYOK AI rename: dispatches chunks of files to a user-configured OpenAI-compatible
//! endpoint via the `aisdk` crate, and lenient-parses the JSON the model returns.
//!
//! No `response_format`/structured-output is ever requested (see
//! `docs/byok-ai-rename-plan.md`), so providers/models that would hard-reject that field
//! still work — we rely entirely on `extract_results` to recover JSON from a plain text
//! response (fenced in ```json, wrapped in prose, or clean).

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use aisdk::core::{DynamicModel, LanguageModelRequest};
use aisdk::providers::OpenAICompatible;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};

use crate::settings::ProviderProfile;
use crate::types::{AiGenerateReport, AiResultItem, FileEntry};

/// Placeholder sent when a profile has no key configured (e.g. local Ollama / LM Studio),
/// since `aisdk`'s `OpenAICompatible` builder rejects an empty `api_key` even though these
/// local servers never check the `Authorization` header.
const NO_KEY_PLACEHOLDER: &str = "not-needed";

pub async fn generate(
    cfg: &ProviderProfile,
    api_key: &str,
    prompt: String,
    entries: Vec<FileEntry>,
) -> Result<AiGenerateReport, String> {
    let key = if api_key.is_empty() {
        NO_KEY_PLACEHOLDER
    } else {
        api_key
    };
    let provider = OpenAICompatible::<DynamicModel>::builder()
        .base_url(&cfg.base_url)
        .api_key(key)
        .model_name(&cfg.model)
        .build()
        .map_err(|e| e.to_string())?;

    let chunk_size = (cfg.chunk_size as usize).max(1);
    let concurrency = (cfg.concurrency as usize).max(1);
    let timeout = Duration::from_secs(cfg.timeout_secs.max(1) as u64);
    let system = system_prompt(cfg.max_len);

    let chunks = chunk_entries(entries, chunk_size);
    let total_chunks = chunks.len();

    log::info!(
        "ai::generate: {} chunk(s), base_url={}, model={}",
        total_chunks,
        cfg.base_url,
        cfg.model
    );
    log::trace!("ai::generate: prompt={prompt}");

    let tasks = chunks.into_iter().enumerate().map(|(chunk_index, chunk)| {
        let provider = provider.clone();
        let system = system.clone();
        let user = user_prompt(&prompt, &chunk, chunk_index * chunk_size);
        let ids: HashSet<String> = chunk.iter().map(|e| e.id.clone()).collect();
        let exts: HashMap<String, String> =
            chunk.iter().map(|e| (e.id.clone(), e.ext.clone())).collect();
        async move {
            log::debug!("ai::generate: chunk {chunk_index} dispatching");
            let outcome = run_chunk(provider, system, user, timeout)
                .await
                .map(|items| reconcile(items, &ids, &exts));
            if let Err(e) = &outcome {
                log::warn!("ai::generate: chunk {chunk_index} failed: {e}");
            }
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
        return Err(first_error.unwrap_or_else(|| "AI generation failed".to_string()));
    }

    let warning = (failed_chunks > 0).then(|| {
        format!(
            "Suggested {} name(s); {} of {} batch(es) failed.",
            results.len(),
            failed_chunks,
            total_chunks
        )
    });

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

async fn run_chunk(
    provider: OpenAICompatible<DynamicModel>,
    system: String,
    prompt: String,
    timeout: Duration,
) -> Result<Vec<AiResultItem>, String> {
    let attempt = async {
        let mut req = LanguageModelRequest::builder()
            .model(provider)
            .system(system)
            .prompt(prompt)
            .temperature(0u32)
            .build();
        req.generate_text().await
    };

    match tokio::time::timeout(timeout, attempt).await {
        Err(_) => Err(format!("Timed out after {}s", timeout.as_secs())),
        Ok(Err(e)) => Err(friendly_error(e)),
        Ok(Ok(resp)) => {
            let text = resp
                .text()
                .ok_or_else(|| "Model returned no text content".to_string())?;
            log::trace!("ai::generate: response={text}");
            extract_results(&text)
        }
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
fn extract_results(text: &str) -> Result<Vec<AiResultItem>, String> {
    let trimmed = text.trim();

    if let Ok(w) = serde_json::from_str::<Wrapped>(trimmed) {
        return Ok(w.results);
    }
    if let Ok(items) = serde_json::from_str::<Vec<AiResultItem>>(trimmed) {
        return Ok(items);
    }

    let start = trimmed.find(['{', '[']);
    let end = trimmed.rfind(['}', ']']);
    if let (Some(s), Some(e)) = (start, end) {
        if e > s {
            let sub = &trimmed[s..=e];
            if let Ok(w) = serde_json::from_str::<Wrapped>(sub) {
                return Ok(w.results);
            }
            if let Ok(items) = serde_json::from_str::<Vec<AiResultItem>>(sub) {
                return Ok(items);
            }
        }
    }

    Err(format!("Could not parse model response as JSON: {trimmed}"))
}

/// Drop hallucinated ids and sanitize each `newName` (no path separators, no leaked
/// extension). Duplicate ids are left as-is; the engine's `HashMap` collect keeps the last
/// occurrence, so order here is preserved from the model's own (chunk-local) response order.
fn reconcile(
    items: Vec<AiResultItem>,
    ids: &HashSet<String>,
    exts: &HashMap<String, String>,
) -> Vec<AiResultItem> {
    items
        .into_iter()
        .filter(|item| ids.contains(&item.id))
        .map(|item| {
            let ext = exts.get(&item.id).map(String::as_str).unwrap_or("");
            AiResultItem {
                new_name: sanitize_name(&item.new_name, ext),
                id: item.id,
            }
        })
        .collect()
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
         keep under {max_len} chars; don't invent ids; if unsure, echo the original name."
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
        let got = extract_results(r#"{"results":[{"id":"a","newName":"A"}]}"#).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id, "a");
        assert_eq!(got[0].new_name, "A");
    }

    #[test]
    fn extract_results_bare_array() {
        let got = extract_results(r#"[{"id":"a","newName":"A"}]"#).unwrap();
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn extract_results_fenced() {
        let got = extract_results("```json\n{\"results\":[{\"id\":\"a\",\"newName\":\"A\"}]}\n```")
            .unwrap();
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn extract_results_prose_wrapped() {
        let got = extract_results(
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
        let got = reconcile(items, &ids, &exts);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].new_name, "New Name");
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
        let err = generate(&cfg, "sk-test", "rename".to_string(), entries)
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
        let report = generate(&cfg, "sk-test", "rename".to_string(), entries)
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
        let err = generate(&cfg, "sk-test", "rename".to_string(), entries)
            .await
            .unwrap_err();
        assert!(err.contains("Timed out"));
    }
}
