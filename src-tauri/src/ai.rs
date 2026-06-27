//! Client for the (future) AI rename backend. Stage 1 only defines and calls the
//! contract; a local mock backend (`mock-backend/server.mjs`) implements it for dev.
//!
//! The backend URL comes from the `LOTF_BACKEND_URL` env var, defaulting to the mock.

use serde::{Deserialize, Serialize};

use crate::types::{AiResultItem, FileEntry};

const DEFAULT_URL: &str = "http://localhost:8787/v1/rename";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestFile {
    id: String,
    name: String,
    ext: String,
    parent_hint: String,
    index: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Options {
    max_len: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    version: u32,
    prompt: String,
    files: Vec<RequestFile>,
    options: Options,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResponseError {
    code: String,
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    #[allow(dead_code)]
    version: u32,
    results: Vec<AiResultItem>,
    #[serde(default)]
    error: Option<ResponseError>,
}

pub async fn generate(
    prompt: String,
    entries: Vec<FileEntry>,
    max_len: u32,
) -> Result<Vec<AiResultItem>, String> {
    let url = std::env::var("LOTF_BACKEND_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());

    let files = entries
        .iter()
        .enumerate()
        .map(|(index, e)| RequestFile {
            id: e.id.clone(),
            name: e.stem.clone(),
            ext: e.ext.clone(),
            parent_hint: e.parent_dir.clone(),
            index,
        })
        .collect();

    let body = Request {
        version: 1,
        prompt,
        files,
        options: Options { max_len },
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Could not reach AI backend at {url}: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("AI backend returned status {}", resp.status()));
    }

    let parsed: Response = resp
        .json()
        .await
        .map_err(|e| format!("Invalid AI backend response: {e}"))?;

    if let Some(err) = parsed.error {
        return Err(format!("AI backend error [{}]: {}", err.code, err.message));
    }
    Ok(parsed.results)
}
