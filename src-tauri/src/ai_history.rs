//! Persistent history of AI rename requests, mirroring `history.rs`'s structure: full
//! request/response detail (including raw model output) is recorded for every generation —
//! success or failure — so past runs stay inspectable from the AI History panel and from a
//! step's "Details" dialog.
//!
//! Shares the same SQLite connection/file as rename history (`HistoryDb`, `history.db`); this
//! module only owns its own tables.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::types::{AiChunkDetail, AiGenerateReport};

/// Number of most-recent generations to retain; raw model responses can be sizeable text blobs
/// (unlike rename history's small rows), so unbounded growth here is a real footgun.
const MAX_RETAINED_GENERATIONS: usize = 200;

pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ai_generations (
            id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL,
            profile_id TEXT NOT NULL,
            profile_label TEXT NOT NULL,
            base_url TEXT NOT NULL,
            model TEXT NOT NULL,
            instruction TEXT NOT NULL,
            system_prompt TEXT NOT NULL,
            entry_count INTEGER NOT NULL,
            chunk_size INTEGER NOT NULL,
            concurrency INTEGER NOT NULL,
            timeout_secs INTEGER NOT NULL,
            max_len INTEGER NOT NULL,
            temperature REAL NOT NULL,
            total_chunks INTEGER NOT NULL,
            failed_chunks INTEGER NOT NULL,
            warning TEXT,
            error TEXT,
            mock INTEGER NOT NULL,
            has_key INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS ai_chunks (
            gen_id TEXT NOT NULL,
            chunk_index INTEGER NOT NULL,
            file_count INTEGER NOT NULL,
            user_prompt TEXT NOT NULL,
            raw_response TEXT,
            error TEXT,
            parse_path TEXT,
            elapsed_ms INTEGER NOT NULL,
            model_count INTEGER,
            dropped_unknown INTEGER,
            sanitized_count INTEGER,
            missing_ids TEXT NOT NULL DEFAULT '[]',
            FOREIGN KEY (gen_id) REFERENCES ai_generations(id)
        );
        CREATE INDEX IF NOT EXISTS idx_ai_chunks_gen ON ai_chunks(gen_id);",
    )
}

/// Row shown in the AI History list: enough to identify and triage a past run without loading
/// every chunk's prompt/response text.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AiGenerationSummary {
    pub id: String,
    pub created_at: String,
    pub profile_label: String,
    pub model: String,
    pub instruction: String,
    pub entry_count: i64,
    pub total_chunks: i64,
    pub failed_chunks: i64,
    pub warning: Option<String>,
    pub error: Option<String>,
    pub mock: bool,
    /// "ok" | "partial" | "failed", derived from `error`/`failed_chunks`.
    pub status: String,
}

fn derive_status(error: &Option<String>, failed_chunks: i64) -> String {
    if error.is_some() {
        "failed".to_string()
    } else if failed_chunks > 0 {
        "partial".to_string()
    } else {
        "ok".to_string()
    }
}

/// Full detail of one generation: its summary plus the exact config sent and every chunk's
/// request/response.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AiGenerationDetail {
    #[serde(flatten)]
    pub summary: AiGenerationSummary,
    pub base_url: String,
    pub system_prompt: String,
    pub chunk_size: i64,
    pub concurrency: i64,
    pub timeout_secs: i64,
    pub max_len: i64,
    pub temperature: f64,
    pub has_key: bool,
    pub chunks: Vec<AiChunkDetail>,
}

/// Records one generation (its metadata row + all chunk rows) in a single transaction, then
/// prunes old generations beyond `MAX_RETAINED_GENERATIONS`.
pub fn record_generation(conn: &Connection, report: &AiGenerateReport) -> rusqlite::Result<()> {
    let req = &report.request;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO ai_generations (
            id, created_at, profile_id, profile_label, base_url, model, instruction,
            system_prompt, entry_count, chunk_size, concurrency, timeout_secs, max_len,
            temperature, total_chunks, failed_chunks, warning, error, mock, has_key
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        params![
            req.generation_id,
            req.created_at,
            req.profile_id,
            req.profile_label,
            req.base_url,
            req.model,
            req.instruction,
            req.system_prompt,
            req.entry_count as i64,
            req.chunk_size as i64,
            req.concurrency as i64,
            req.timeout_secs as i64,
            req.max_len as i64,
            req.temperature as f64,
            report.total_chunks as i64,
            report.failed_chunks as i64,
            report.warning,
            report.error,
            req.mock,
            req.has_key,
        ],
    )?;
    for chunk in &report.chunks {
        let missing_ids = serde_json::to_string(&chunk.missing_ids).unwrap_or_else(|_| "[]".to_string());
        tx.execute(
            "INSERT INTO ai_chunks (
                gen_id, chunk_index, file_count, user_prompt, raw_response, error, parse_path,
                elapsed_ms, model_count, dropped_unknown, sanitized_count, missing_ids
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                req.generation_id,
                chunk.chunk_index as i64,
                chunk.file_count as i64,
                chunk.user_prompt,
                chunk.raw_response,
                chunk.error,
                chunk.parse_path,
                chunk.elapsed_ms as i64,
                chunk.model_count.map(|n| n as i64),
                chunk.dropped_unknown.map(|n| n as i64),
                chunk.sanitized_count.map(|n| n as i64),
                missing_ids,
            ],
        )?;
    }
    tx.commit()?;
    prune_ai_generations(conn, MAX_RETAINED_GENERATIONS)
}

/// Deletes generations beyond the most recent `keep` (by `created_at`), plus their chunk rows.
pub fn prune_ai_generations(conn: &Connection, keep: usize) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM ai_chunks WHERE gen_id NOT IN (
            SELECT id FROM ai_generations ORDER BY created_at DESC LIMIT ?1
        )",
        params![keep as i64],
    )?;
    conn.execute(
        "DELETE FROM ai_generations WHERE id NOT IN (
            SELECT id FROM ai_generations ORDER BY created_at DESC LIMIT ?1
        )",
        params![keep as i64],
    )?;
    Ok(())
}

pub fn list_ai_generations(conn: &Connection) -> rusqlite::Result<Vec<AiGenerationSummary>> {
    let mut stmt = conn.prepare(
        "SELECT id, created_at, profile_label, model, instruction, entry_count, total_chunks,
                failed_chunks, warning, error, mock
         FROM ai_generations
         ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            let error: Option<String> = r.get(9)?;
            let failed_chunks: i64 = r.get(7)?;
            Ok(AiGenerationSummary {
                id: r.get(0)?,
                created_at: r.get(1)?,
                profile_label: r.get(2)?,
                model: r.get(3)?,
                instruction: r.get(4)?,
                entry_count: r.get(5)?,
                total_chunks: r.get(6)?,
                failed_chunks,
                warning: r.get(8)?,
                status: derive_status(&error, failed_chunks),
                error,
                mock: r.get(10)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get_ai_generation(conn: &Connection, id: &str) -> rusqlite::Result<Option<AiGenerationDetail>> {
    let summary = conn
        .query_row(
            "SELECT id, created_at, profile_label, model, instruction, entry_count, total_chunks,
                    failed_chunks, warning, error, mock, base_url, system_prompt, chunk_size,
                    concurrency, timeout_secs, max_len, temperature, has_key
             FROM ai_generations WHERE id = ?1",
            params![id],
            |r| {
                let error: Option<String> = r.get(9)?;
                let failed_chunks: i64 = r.get(7)?;
                let summary = AiGenerationSummary {
                    id: r.get(0)?,
                    created_at: r.get(1)?,
                    profile_label: r.get(2)?,
                    model: r.get(3)?,
                    instruction: r.get(4)?,
                    entry_count: r.get(5)?,
                    total_chunks: r.get(6)?,
                    failed_chunks,
                    warning: r.get(8)?,
                    status: derive_status(&error, failed_chunks),
                    error,
                    mock: r.get(10)?,
                };
                Ok((
                    summary,
                    r.get::<_, String>(11)?,
                    r.get::<_, String>(12)?,
                    r.get::<_, i64>(13)?,
                    r.get::<_, i64>(14)?,
                    r.get::<_, i64>(15)?,
                    r.get::<_, i64>(16)?,
                    r.get::<_, f64>(17)?,
                    r.get::<_, bool>(18)?,
                ))
            },
        )
        .optional()?;

    let Some((summary, base_url, system_prompt, chunk_size, concurrency, timeout_secs, max_len, temperature, has_key)) =
        summary
    else {
        return Ok(None);
    };

    let mut stmt = conn.prepare(
        "SELECT chunk_index, file_count, user_prompt, raw_response, error, parse_path,
                elapsed_ms, model_count, dropped_unknown, sanitized_count, missing_ids
         FROM ai_chunks WHERE gen_id = ?1 ORDER BY chunk_index",
    )?;
    let chunks = stmt
        .query_map(params![id], |r| {
            let missing_ids_json: String = r.get(10)?;
            let missing_ids: Vec<String> = serde_json::from_str(&missing_ids_json).unwrap_or_default();
            Ok(AiChunkDetail {
                chunk_index: r.get::<_, i64>(0)? as usize,
                file_count: r.get::<_, i64>(1)? as usize,
                user_prompt: r.get(2)?,
                raw_response: r.get(3)?,
                error: r.get(4)?,
                parse_path: r.get(5)?,
                elapsed_ms: r.get::<_, i64>(6)? as u64,
                model_count: r.get::<_, Option<i64>>(7)?.map(|n| n as usize),
                dropped_unknown: r.get::<_, Option<i64>>(8)?.map(|n| n as usize),
                sanitized_count: r.get::<_, Option<i64>>(9)?.map(|n| n as usize),
                missing_ids,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(Some(AiGenerationDetail {
        summary,
        base_url,
        system_prompt,
        chunk_size,
        concurrency,
        timeout_secs,
        max_len,
        temperature,
        has_key,
        chunks,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AiRequestMeta;

    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    fn sample_report(id: &str) -> AiGenerateReport {
        AiGenerateReport {
            results: vec![],
            failed_chunks: 1,
            total_chunks: 2,
            warning: Some("partial".to_string()),
            error: None,
            request: AiRequestMeta {
                generation_id: id.to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                profile_id: "p1".to_string(),
                profile_label: "Test Provider".to_string(),
                base_url: "https://example.com/v1".to_string(),
                model: "gpt-test".to_string(),
                instruction: "rename these".to_string(),
                system_prompt: "you rename files".to_string(),
                entry_count: 3,
                chunk_size: 40,
                concurrency: 3,
                timeout_secs: 60,
                max_len: 300,
                temperature: 0.0,
                mock: false,
                has_key: true,
            },
            chunks: vec![
                AiChunkDetail {
                    chunk_index: 0,
                    file_count: 2,
                    user_prompt: "Instruction: rename these\n\nFiles: [...]".to_string(),
                    raw_response: Some(r#"{"results":[{"id":"a","newName":"Alpha"}]}"#.to_string()),
                    error: None,
                    parse_path: Some("wrapped_object".to_string()),
                    elapsed_ms: 120,
                    model_count: Some(1),
                    dropped_unknown: Some(0),
                    sanitized_count: Some(0),
                    missing_ids: vec!["b".to_string()],
                },
                AiChunkDetail {
                    chunk_index: 1,
                    file_count: 1,
                    user_prompt: "Instruction: rename these\n\nFiles: [...]".to_string(),
                    raw_response: None,
                    error: Some("Provider rejected the API key".to_string()),
                    parse_path: None,
                    elapsed_ms: 30,
                    model_count: None,
                    dropped_unknown: None,
                    sanitized_count: None,
                    missing_ids: vec![],
                },
            ],
        }
    }

    #[test]
    fn record_then_list_round_trips() {
        let conn = mem_db();
        record_generation(&conn, &sample_report("gen-1")).unwrap();

        let list = list_ai_generations(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "gen-1");
        assert_eq!(list[0].status, "partial");
        assert_eq!(list[0].profile_label, "Test Provider");
    }

    #[test]
    fn record_then_get_includes_chunks_with_raw_response_and_error() {
        let conn = mem_db();
        record_generation(&conn, &sample_report("gen-1")).unwrap();

        let detail = get_ai_generation(&conn, "gen-1").unwrap().unwrap();
        assert_eq!(detail.base_url, "https://example.com/v1");
        assert_eq!(detail.chunks.len(), 2);
        assert_eq!(
            detail.chunks[0].raw_response.as_deref(),
            Some(r#"{"results":[{"id":"a","newName":"Alpha"}]}"#)
        );
        assert_eq!(detail.chunks[0].missing_ids, vec!["b".to_string()]);
        assert!(detail.chunks[1].raw_response.is_none());
        assert_eq!(detail.chunks[1].error.as_deref(), Some("Provider rejected the API key"));
    }

    #[test]
    fn get_unknown_id_returns_none() {
        let conn = mem_db();
        assert!(get_ai_generation(&conn, "nope").unwrap().is_none());
    }

    #[test]
    fn status_derives_failed_when_error_set() {
        let conn = mem_db();
        let mut report = sample_report("gen-failed");
        report.error = Some("all chunks failed".to_string());
        record_generation(&conn, &report).unwrap();

        let list = list_ai_generations(&conn).unwrap();
        assert_eq!(list[0].status, "failed");
    }

    #[test]
    fn status_derives_ok_when_no_failures() {
        let conn = mem_db();
        let mut report = sample_report("gen-ok");
        report.failed_chunks = 0;
        report.warning = None;
        record_generation(&conn, &report).unwrap();

        let list = list_ai_generations(&conn).unwrap();
        assert_eq!(list[0].status, "ok");
    }

    #[test]
    fn prune_keeps_only_most_recent() {
        let conn = mem_db();
        for i in 0..5 {
            let mut report = sample_report(&format!("gen-{i}"));
            report.request.created_at = format!("2024-01-0{}T00:00:00Z", i + 1);
            record_generation(&conn, &report).unwrap();
        }
        prune_ai_generations(&conn, 2).unwrap();

        let list = list_ai_generations(&conn).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "gen-4");
        assert_eq!(list[1].id, "gen-3");

        // Orphaned chunk rows were cleaned up too.
        let chunk_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM ai_chunks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(chunk_count, 4); // 2 retained generations × 2 chunks each
    }
}
