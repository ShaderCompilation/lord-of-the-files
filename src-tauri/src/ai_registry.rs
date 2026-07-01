//! Tauri-managed registry of in-flight AI generations, so a superseded or user-cancelled
//! generation can actually stop its backend work instead of just being ignored by the
//! frontend. Mirrors the `HistoryDb`/`SettingsDb` `Mutex<...>`-in-managed-state pattern.

use std::collections::HashMap;
use std::sync::Mutex;

use tokio_util::sync::CancellationToken;

pub struct AiGenerationRegistry(Mutex<HashMap<String, CancellationToken>>);

impl AiGenerationRegistry {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    /// Registers a fresh token for `generation_id`, overwriting any stale entry. Returns the
    /// token for the caller to pass into `ai::generate`.
    pub fn register(&self, generation_id: String) -> CancellationToken {
        let token = CancellationToken::new();
        if let Ok(mut map) = self.0.lock() {
            map.insert(generation_id, token.clone());
        }
        token
    }

    /// Cancels the token for `generation_id` if present. No-op if the generation already
    /// finished and removed itself — cancelling a request that's about to return its own
    /// result anyway is a harmless, expected race, not a bug.
    pub fn cancel(&self, generation_id: &str) {
        if let Ok(map) = self.0.lock() {
            if let Some(token) = map.get(generation_id) {
                token.cancel();
            }
        }
    }

    /// Removes the entry once a generation is done, so the map doesn't grow unboundedly over
    /// a long session.
    pub fn remove(&self, generation_id: &str) {
        if let Ok(mut map) = self.0.lock() {
            map.remove(generation_id);
        }
    }
}

impl Default for AiGenerationRegistry {
    fn default() -> Self {
        Self::new()
    }
}
