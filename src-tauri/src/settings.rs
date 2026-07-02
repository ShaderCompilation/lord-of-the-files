//! Non-secret provider configuration (SQLite) + secret API keys (OS keychain).
//!
//! Mirrors the `HistoryDb` managed-state pattern (`history.rs`): a `Mutex<Connection>` held
//! in Tauri managed state, opened once at startup. All config lives as one JSON blob under a
//! single settings row (no per-column migrations). Keys never enter that blob and never cross
//! IPC after entry — only a computed `hasKey` boolean does.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// SQLite connection held in Tauri managed state.
pub struct SettingsDb(pub Mutex<Connection>);

const KEYRING_SERVICE: &str = "com.lordofthefiles.app";

fn default_chunk_size() -> u32 {
    40
}
fn default_concurrency() -> u32 {
    3
}
fn default_max_len() -> u32 {
    300
}
fn default_timeout_secs() -> u32 {
    60
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfile {
    pub id: String,
    pub label: String,
    pub base_url: String,
    pub model: String,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: u32,
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    #[serde(default = "default_max_len")]
    pub max_len: u32,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u32,
    /// Always recomputed from the keychain before crossing IPC; never persisted meaningfully
    /// and never holds the key itself.
    #[serde(default)]
    pub has_key: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SettingsState {
    pub profiles: Vec<ProviderProfile>,
    pub active_profile_id: Option<String>,
    #[serde(default)]
    pub debug_logging: bool,
    #[serde(default)]
    pub mock_ai: MockAiConfig,
}

/// How a mocked chunk turns each file's stem into a "suggestion" — see [`MockAiConfig`].
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MockTransform {
    #[default]
    Suffix,
    Uppercase,
    Lowercase,
    Reverse,
    Slugify,
}

fn default_mock_latency_ms() -> u32 {
    500
}

/// Dev-only simulated AI backend, so the BYOK rename flow (chunking, progress events,
/// partial/full-failure handling, reconciliation) can be exercised without a real provider or
/// API cost. Persisted like any other setting (so the Dev menu survives a restart), but
/// `ai::generate` only ever honours `enabled` inside `cfg!(debug_assertions)` builds — a
/// release binary ignores it even if a dev build previously left it on in the same settings.db.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MockAiConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_mock_latency_ms")]
    pub latency_ms: u32,
    /// Chance (0.0-1.0) that any given chunk simulates a provider failure.
    #[serde(default)]
    pub fail_rate: f32,
    #[serde(default)]
    pub transform: MockTransform,
}

impl Default for MockAiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            latency_ms: default_mock_latency_ms(),
            fail_rate: 0.0,
            transform: MockTransform::default(),
        }
    }
}

pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )
}

pub fn load_state(conn: &Connection) -> SettingsState {
    let raw: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = 'state'", [], |r| {
            r.get(0)
        })
        .optional()
        .unwrap_or(None);
    raw.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_state(conn: &Connection, state: &SettingsState) -> Result<(), String> {
    let json = serde_json::to_string(state).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES ('state', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![json],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn keychain_unavailable(e: keyring::Error) -> String {
    log::warn!("keychain unavailable: {e}");
    format!(
        "No OS keychain available ({e}) — install gnome-keyring/KWallet, or set LOTF_API_KEY"
    )
}

/// Reads the key from the OS keychain; falls back to `LOTF_API_KEY` when the keychain has no
/// entry for this profile (headless/CI, or no keychain backend installed).
pub fn get_api_key(profile_id: &str) -> Option<String> {
    match keyring::Entry::new(KEYRING_SERVICE, profile_id) {
        Ok(entry) => match entry.get_password() {
            Ok(pw) => Some(pw),
            Err(_) => std::env::var("LOTF_API_KEY").ok(),
        },
        Err(_) => std::env::var("LOTF_API_KEY").ok(),
    }
}

pub fn set_api_key(profile_id: &str, key: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, profile_id).map_err(keychain_unavailable)?;
    entry.set_password(key).map_err(keychain_unavailable)?;
    log::debug!("key set for {profile_id}");
    Ok(())
}

pub fn clear_api_key(profile_id: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, profile_id).map_err(keychain_unavailable)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {
            log::debug!("key cleared for {profile_id}");
            Ok(())
        }
        Err(e) => Err(keychain_unavailable(e)),
    }
}

pub fn has_api_key(profile_id: &str) -> bool {
    get_api_key(profile_id).is_some_and(|k| !k.is_empty())
}

/// Rejects any URL scheme other than http/https, so a compromised frontend can't point a
/// profile's `base_url` at a `file://`, `javascript:`, `data:`, or other non-network scheme
/// that `ai::generate`/`test_connection` would otherwise build a client against — the
/// `Authorization` header (and the key it may carry) would go wherever `base_url` points.
/// Deliberately permissive about *host*: localhost/private IPs are legitimate (the Ollama and
/// LM Studio presets in `src/lib/providers.ts` both use `http://localhost:...`); only the
/// scheme is restricted.
pub fn validate_base_url(base_url: &str) -> Result<(), String> {
    let url = url::Url::parse(base_url.trim()).map_err(|e| format!("Invalid provider URL: {e}"))?;
    match url.scheme() {
        "http" | "https" => Ok(()),
        other => Err(format!("Provider URL must use http:// or https:// (got \"{other}:\")")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn validate_base_url_accepts_http_and_https() {
        assert!(validate_base_url("https://api.openai.com/v1").is_ok());
        assert!(validate_base_url("http://localhost:11434/v1").is_ok()); // Ollama-style
    }

    #[test]
    fn validate_base_url_rejects_non_http_schemes() {
        assert!(validate_base_url("file:///etc/passwd").is_err());
        assert!(validate_base_url("javascript:alert(1)").is_err());
        assert!(validate_base_url("data:text/plain,hello").is_err());
        assert!(validate_base_url("ftp://example.com").is_err());
    }

    #[test]
    fn validate_base_url_rejects_empty_or_malformed() {
        assert!(validate_base_url("").is_err());
        assert!(validate_base_url("not a url").is_err());
    }

    #[test]
    fn load_state_defaults_when_missing() {
        let conn = mem_db();
        let state = load_state(&conn);
        assert!(state.profiles.is_empty());
        assert!(state.active_profile_id.is_none());
    }

    #[test]
    fn save_then_load_round_trips() {
        let conn = mem_db();
        let state = SettingsState {
            profiles: vec![ProviderProfile {
                id: "p1".into(),
                label: "OpenRouter".into(),
                base_url: "https://openrouter.ai/api/v1".into(),
                model: "openai/gpt-4o-mini".into(),
                chunk_size: 40,
                concurrency: 3,
                max_len: 300,
                timeout_secs: 60,
                has_key: false,
            }],
            active_profile_id: Some("p1".into()),
            debug_logging: true,
            mock_ai: MockAiConfig {
                enabled: true,
                latency_ms: 250,
                fail_rate: 0.25,
                transform: MockTransform::Uppercase,
            },
        };
        save_state(&conn, &state).unwrap();
        let loaded = load_state(&conn);
        assert_eq!(loaded.active_profile_id.as_deref(), Some("p1"));
        assert_eq!(loaded.profiles.len(), 1);
        assert_eq!(loaded.profiles[0].label, "OpenRouter");
        assert!(loaded.debug_logging);
        assert_eq!(loaded.mock_ai, state.mock_ai);
    }

    #[test]
    fn debug_logging_defaults_to_false_when_blob_missing_key() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('state', '{\"profiles\":[],\"activeProfileId\":null}')",
            [],
        )
        .unwrap();
        let loaded = load_state(&conn);
        assert!(!loaded.debug_logging);
    }

    #[test]
    fn mock_ai_defaults_to_disabled_when_blob_missing_key() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('state', '{\"profiles\":[],\"activeProfileId\":null}')",
            [],
        )
        .unwrap();
        let loaded = load_state(&conn);
        assert!(!loaded.mock_ai.enabled);
        assert_eq!(loaded.mock_ai.transform, MockTransform::Suffix);
    }

    #[test]
    #[ignore = "requires a real OS keychain backend"]
    fn keychain_round_trip() {
        let id = "lotf-test-profile";
        set_api_key(id, "sk-test-123").unwrap();
        assert!(has_api_key(id));
        assert_eq!(get_api_key(id).as_deref(), Some("sk-test-123"));
        clear_api_key(id).unwrap();
        assert!(!has_api_key(id));
    }
}
