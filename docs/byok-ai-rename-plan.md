# BYOK AI Rename

## Context

Stage 1's **AI rename** step currently calls a future SaaS backend: the frontend triggers
`ai_generate`, which (`src-tauri/src/ai.rs`) POSTs an unauthenticated `{version,prompt,files,options}`
body to `POST /v1/rename` (`LOTF_BACKEND_URL`, default `http://localhost:8787`), implemented by a Node
mock in `mock-backend/server.mjs`. The Rust engine never calls an LLM — it just caches the returned
`results` on the step and applies them as a `HashMap<id,newName>` lookup during preview/apply.

We are dropping the SaaS model in favor of **BYOK (bring your own key)**: users supply their own LLM
provider keys. Goal: support as many providers as possible with the least effort.

**Decisions (chosen):**
- **One universal adapter: OpenAI-compatible Chat Completions.** A "provider" reduces to `{baseUrl, model, apiKey}`. This single code path covers OpenAI, OpenRouter (one key → hundreds of models), Groq, Together, Fireworks, DeepInfra, Mistral, DeepSeek, xAI, Perplexity, Google Gemini's OpenAI-compat endpoint, and local Ollama / LM Studio / vLLM. Anthropic and Gemini remain reachable via their own compat endpoints or via OpenRouter — no native adapter in v1.
- **Use the `aisdk` crate (`lazy-hq/aisdk`, MIT, feature-gated) as the HTTP transport, not hand-rolled `reqwest`.** It already builds on `reqwest 0.12` + `rustls-tls` + `tokio` — the exact stack this project uses — and ships an `OpenAICompatible<DynamicModel>` provider (`.builder().base_url(..).api_key(..).model_name(..).build()`) that is exactly the "any OpenAI-compatible endpoint" shape this adapter needs. Enable only the `openai_compatible` Cargo feature (providers are feature-gated; the other 74 are not compiled in). Its structured-output (`schema`/`response_format`) support is optional per-request — **we deliberately never set it**, and instead keep the prompt-based "ask for JSON in the response" contract with our own lenient parser. This sidesteps the risk of some OpenAI-compatible providers/models hard-rejecting `response_format` (which would defeat "least effort, most providers" for exactly the providers this is meant to support) — since we never send that field, there's nothing for an incompatible provider to reject.
- **Keep the HTTP call in Rust.** Avoids webview CORS and keeps the key out of JS after entry.
- **Key storage: OS keychain via the `keyring` crate**, with an `LOTF_API_KEY` env fallback for headless/CI. Non-secret config (profiles, baseUrl, model) lives in a small SQLite settings table.
- **Multiple saved profiles**, one active at a time; switch from a dropdown. The key never crosses IPC after entry — the UI only ever learns `hasKey: bool`.

## Approach

Generation stays a one-shot, cached, frontend-triggered Rust call returning `Vec<AiResultItem>`, so
**the engine and the `Step::Ai`/`AiResultItem` types are untouched** (`src-tauri/src/types.rs:79,133`).
We only swap the SaaS POST for a provider dispatch and add a settings/keychain layer + a Settings UI.

---

## Rust (`src-tauri`)

### `Cargo.toml` (modify)
Add:
- `keyring = { version = "3", features = ["apple-native", "windows-native", "sync-secret-service", "crypto-rust"] }` — OS-native backends, no extra C deps on Linux.
- `futures = "0.3"` — bounded-concurrency chunk dispatch (`buffer_unordered`). (`aisdk` also depends on `futures` transitively, but keep this as an explicit direct dep since `ai.rs` uses it directly.)
- `aisdk = { version = "0.5", default-features = false, features = ["openai_compatible"] }` — provides the `OpenAICompatible<DynamicModel>` provider used in `ai.rs` below; only the `openai_compatible` feature is enabled so the other 74 provider modules aren't compiled in.

### `src/settings.rs` (create)
Owns non-secret config (SQLite) + secret keys (keyring). Mirror the `HistoryDb` managed-state pattern
(`lib.rs:29`). All IPC types `#[serde(rename_all = "camelCase")]`.

```
pub struct SettingsDb(pub Mutex<Connection>);

pub struct ProviderProfile {           // serde camelCase
    id: String, label: String,
    base_url: String, model: String,
    chunk_size: u32,   // default 40
    concurrency: u32,  // default 3
    max_len: u32,      // default 80 (replaces the hardcoded 80)
    timeout_secs: u32, // default 60; aisdk has no per-request timeout knob (only max_retries),
                       // so ai.rs wraps each generate_text() call in tokio::time::timeout(..)
    has_key: bool,    // computed in command layer; NEVER stores/returns the key
}
pub struct SettingsState { profiles: Vec<ProviderProfile>, active_profile_id: Option<String> }

pub fn init_schema(conn)                 // CREATE TABLE settings(key TEXT PRIMARY KEY, value TEXT)
pub fn load_state(conn) -> SettingsState // row "state" -> serde_json; default empty if missing
pub fn save_state(conn, &SettingsState)  // upsert single JSON blob (no key material)

// keyring, account = profile id, service = "com.lordofthefiles.app":
const KEYRING_SERVICE: &str = "com.lordofthefiles.app";
pub fn get_api_key(profile_id) -> Option<String>  // keyring Entry::get_password, else env LOTF_API_KEY
pub fn set_api_key(profile_id, &str) -> Result<(),String>
pub fn clear_api_key(profile_id) -> Result<(),String>
pub fn has_api_key(profile_id) -> bool
```
Store the whole `SettingsState` as one JSON blob under settings key `"state"` (no per-column migrations).
Wrap every keyring call; on failure return an actionable message ("No OS keychain available — install
gnome-keyring/KWallet, or set `LOTF_API_KEY`"). Never silently fall back to plaintext.

### `src/ai.rs` (rewrite — core change)
Replace the SaaS POST with the `aisdk`-backed OpenAI-compatible adapter + chunking + lenient parse.
**Keep the public return shape** so the engine cache path is unchanged.

```
pub async fn generate(cfg: &ProviderProfile, api_key: &str, prompt: String,
                      entries: Vec<FileEntry>) -> Result<AiGenerateReport, String>
```
- **Build one provider instance per call** (not per chunk — `base_url`/`api_key`/`model` don't vary
  within a call): `aisdk::providers::OpenAICompatible::<aisdk::core::DynamicModel>::builder()
  .base_url(&cfg.base_url).api_key(api_key).model_name(&cfg.model).build()`. It's `Clone`, so
  `.clone()` (or wrap in `Arc`) it per concurrent chunk task.
- **Chunk** entries into `cfg.chunk_size` slices (pure helper `chunk_entries(&[FileEntry], usize)` so it's unit-testable). Dispatch via `futures::stream::iter(...).buffer_unordered(cfg.concurrency)`; each yields `Result<Vec<AiResultItem>, String>`.
- **Per-chunk request:** `aisdk::core::LanguageModelRequest::builder().model(provider.clone())
  .system(SYSTEM_PROMPT).prompt(chunk_json).temperature(0.0).build()?.generate_text()`, wrapped in
  `tokio::time::timeout(Duration::from_secs(cfg.timeout_secs), ..)` (an elapsed timeout counts as a
  chunk failure, same as the rest of this list). **Deliberately omit `.schema(...)`** — no structured
  output / `response_format` is requested, so there's nothing for an incompatible provider to reject;
  we rely entirely on the lenient parser below. Map `aisdk::Error` to `String` via `.map_err(|e| e.to_string())`.
- **Parse** the returned text (a JSON string, per the prompt contract) via lenient `extract_results(&str) -> Result<Vec<AiResultItem>>`: try `{"results":[...]}`, then a bare array, then first-`{`/`[` … last-`}`/`]` substring (handles ```json fences / prose).
- **Reconcile:** build the input-id set; **drop** results with unknown ids (hallucinations); **sanitize** each `newName` (strip path separators; strip a trailing `.{ext}` if it equals the file's known ext). Missing ids need no handling — the engine already falls back to the current name (`engine/mod.rs:185`). Duplicate ids: last-wins (matches the engine's map).
- **Merge:** concat oks, count errs → `AiGenerateReport { results, failed_chunks, total_chunks, warning }`. If every chunk failed, return `Err`.

### `src/commands.rs` (modify)
Replace `ai_generate` and add settings commands:
```
async ai_generate(settings_db: State<SettingsDb>, prompt, entries) -> Result<AiGenerateReport,String>
   // load active profile + its key; err "No active provider / no key — open Settings" if missing
get_settings(db) -> Result<SettingsState,String>          // fills has_key per profile via has_api_key
upsert_profile(db, profile: ProviderProfile) -> Result<(),String>
delete_profile(db, id: String) -> Result<(),String>        // also clear_api_key(id); if id ==
                                                             // active_profile_id, clear active_profile_id
                                                             // to None (don't leave it dangling)
set_active_profile(db, id: String) -> Result<(),String>
set_api_key(profile_id: String, key: String) -> Result<(),String>
clear_api_key(profile_id: String) -> Result<(),String>
async test_connection(db: State<SettingsDb>, profile_id: String) -> Result<String,String>
   // tiny 1-file chat completion exercising auth+url+model+JSON; returns "ok" or a clear error
```
`AiGenerateReport` is a new `#[serde(rename_all="camelCase")]` struct in `types.rs`.

### `src/lib.rs` (modify)
- `mod settings;`
- In `.setup`: open a second `Connection` to `settings.db` (sibling of `history.db`), `settings::init_schema`, `app.manage(SettingsDb(Mutex::new(conn)))`.
- Register the new/updated commands in `generate_handler![…]`.

### `types.rs` (modify)
- `Step::Ai` / `AiResultItem` — unchanged.
- Add `AiGenerateReport`. (`ProviderProfile`/`SettingsState` live in `settings.rs` but cross IPC, so keep them camelCase.)

### Rust tests (`#[cfg(test)]`)
- `ai.rs`: `extract_results` (clean object, bare array, fenced, prose-wrapped, garbage→Err); `chunk_entries` (95 files / size 40 → 3); HTTP-level test against a mocked `/chat/completions` server (add `httpmock` or `wiremock` as dev-dep, point `cfg.base_url` at it — `aisdk`'s `OpenAICompatible` just needs a reachable base URL) incl. one chunk 500 → partial result + `failed_chunks>0`; one chunk exceeding `timeout_secs` → counted as a chunk failure; reconciliation drops unknown ids + sanitizes extensions.
- `settings.rs`: `load_state` default when missing; `save_state`→`load_state` round-trip. Gate any real-keychain test behind `#[ignore]`.

---

## Frontend (`src`)

### `lib/types.ts` (modify)
Remove obsolete `AiRequestFile` / `AiResponse`. Keep `AiResultItem`. Add `ProviderProfile`,
`SettingsState`, `AiGenerateReport` (mirror the Rust camelCase shapes).

### `lib/providers.ts` (create)
Preset list prefilling `baseUrl` + a default model (user-editable; model ids drift — verify at impl):
OpenAI, **OpenRouter** (call out: one key → hundreds of models), Groq, Together, Fireworks, DeepInfra,
Mistral, DeepSeek, xAI (Grok), Perplexity, Gemini (OpenAI-compat), Ollama (local), LM Studio (local),
Custom…. `interface ProviderPreset { label; baseUrl; defaultModel; keyHint? }`.

### `lib/ipc.ts` (modify)
`aiGenerate(prompt, entries) -> Promise<AiGenerateReport>` (drop `maxLen`). Add wrappers for
`getSettings`, `upsertProfile`, `deleteProfile`, `setActiveProfile`, `setApiKey`, `clearApiKey`,
`testConnection` (snake_case command names, camelCase args per `ipc.ts` convention).

### `store.ts` (modify)
- Rewrite `generateAi` (`store.ts:237`): drop the hardcoded `80`; call `ipc.aiGenerate(prompt, entries)`; set `report.results`; surface `report.warning` (e.g. "Suggested 90 name(s); 1 of 3 batches failed."). Pre-check active profile + `hasKey`; if absent, notice → open Settings.
- Add a settings slice: `settings` signal + `loadSettings`, `upsertProfile`, `deleteProfile`, `setActiveProfile`, `saveApiKey`, `clearApiKey`, `testConnection` (each re-loads settings after a mutation). Call `loadSettings()` in `App.tsx` `onMount` alongside the existing history refresh.

### `components/SettingsPanel.tsx` (create)
Mirror `HistoryPanel.tsx` exactly: `props { open, onClose }`, `<Show>` + `.overlay` button + right-side
`<aside>`. Contents:
- **Profiles dropdown** + "Add profile" / "Delete" / "Set active" (active marked). 
- Editor for the selected profile: **preset `<select>`** (prefills baseUrl/model on change), **Base URL**, **Model**, **API key** `<input type="password">` (show "key is set" from `hasKey`; "Change key" clears for re-entry — the secret is never returned), and a collapsible **Advanced** (chunk size, concurrency, max length).
- Buttons: **Save** (`upsertProfile` + `setApiKey` if the field is non-empty), **Clear key**, **Test connection** (spinner → inline ok/error). Reuse `.field` / `.check` / button classes.

### `components/Toolbar.tsx` (modify)
Add a **Settings** button (`class="ghost"`) in the right `toolbar-group` next to History, wired to a new `onToggleSettings` prop (mirror `onToggleHistory`, `Toolbar.tsx:74`).

### `App.tsx` (modify)
Add `settingsOpen` signal; pass `onToggleSettings`; render `<SettingsPanel>` beside `<HistoryPanel>`; `await s.loadSettings()` in `onMount`.

### `components/StepCard.tsx` (modify, small)
In the AI `<Match>` block (~331-359): when no active profile has a key, show a "Set up a provider in Settings" hint and disable Generate. Keep the "N suggestion(s) cached" line.

### `App.css` (modify)
Add `.settings-panel` reusing the `.overlay` + right-side panel rules; minor styling for the masked key field and inline test-connection status.

---

## Prompt + JSON contract sent to the model

**System:** "You rename files. Given a JSON array of files (each `id`, current `name` without
extension, `ext`, `parentHint`) and an instruction, return ONLY `{"results":[{"id","newName"}]}`. Keep
each `id` exactly; one result per file; `newName` is the stem only — never an extension, path, or
separator; keep under {maxLen} chars; don't invent ids; if unsure, echo the original name."
**User:** "Instruction: {prompt}\n\nFiles: {json chunk of [{id,name,ext,parentHint,index}]}".
`temperature:0`; no `response_format`/structured-output request is sent (see Decisions — this keeps
the adapter working against providers/models that don't support that param, relying entirely on the
lenient `extract_results` parser). Extensions are never sent for change nor expected back (stem-only,
matching today's `name: e.stem`); the engine reassembles `stem.ext`.

## Chunking
`chunk_size` 40, `concurrency` 3 (both per-profile, user-editable). Contiguous order-preserving chunks;
`index` in the prompt is the global index. 60s per-chunk timeout → counts as a chunk failure, not a
hard error. Partial results always returned when ≥1 chunk succeeds.

## Cleanup (obsolete SaaS)
Delete `mock-backend/` and its `package.json` script; remove `LOTF_BACKEND_URL`, the `version`
envelope, and `AiRequestFile`/`AiResponse`. Update `README.md`: replace the "AI backend / mock"
section with "BYOK: configure a provider in Settings; presets incl. OpenRouter; use Ollama for offline
dev," and update the `ai.rs` description in the Layout block.

## Implementation order
1. `settings.rs` (SQLite + keyring + env fallback) → wire into `lib.rs`; add `keyring`/`futures`/`aisdk` deps. (cargo test round-trip)
2. `ai.rs` rewrite (`aisdk`-backed adapter + lenient parser + chunking + timeout wrapping) + tests.
3. `commands.rs` (new `ai_generate` + settings/test commands) → register in `lib.rs`.
4. Frontend `types` + `ipc` + `providers.ts`.
5. `store.ts` settings slice + `generateAi` rewrite.
6. `SettingsPanel` + Toolbar/App wiring + StepCard hint + CSS.
7. Delete SaaS bits + README.
8. Verification pass.

## Verification
- `cd src-tauri && cargo test` (new ai/settings tests + existing engine/history pass unchanged), `cargo clippy --all-targets`.
- `pnpm exec tsc --noEmit`; `pnpm tauri dev`.
- **Real key:** Settings → OpenRouter → paste key → Test connection → ok. Add messy files → AI step → "Title Case, human-readable" → Generate → preview shows new stems with original extensions → Apply → Undo. Relaunch → config persists (SQLite) + key persists (keychain).
- **Offline (replaces the deleted mock):** `ollama serve` + `ollama pull llama3.1`; Settings → Ollama preset (`http://localhost:11434/v1`), blank key → Generate. Also the practical test of `timeout_secs`, since local CPU-bound models are the most likely to need longer than the 60s default.
- **Edge cases:** no key (clear error → Settings); 401/403 → "Provider rejected the API key"; provider 5xx → partial results + warning; malformed model JSON (expected, since we never request structured output) → lenient recovery or per-chunk failure (never panics); unknown/missing/duplicate ids handled by reconciliation; extension leakage stripped; deleting the active profile → `active_profile_id` clears and the UI prompts to pick another; Linux without keychain → actionable error + `LOTF_API_KEY` fallback; `cargo build` confirms only the `openai_compatible` feature's transitive deps are pulled in from `aisdk`, not all 75 providers.
- **Coverage note:** only OpenRouter and Ollama get real end-to-end testing above. The other ~11 presets on the provider list are expected to work by construction (same OpenAI-compatible shape, routed through `aisdk`'s `OpenAICompatible` provider) but are **unverified** — treat as a known gap, not a tested guarantee.
