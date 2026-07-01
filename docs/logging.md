# Debug Logging System

## Context

The app (a Tauri v2 desktop file-renamer: SolidJS frontend in `src/`, Rust backend in
`src-tauri/`) currently has **no logging at all** — no `console.*`, no Rust `log`/`tracing`.
Errors are only surfaced transiently via a `setNotice()` banner in `src/store.ts`. When
something goes wrong (a failed AI rename, a botched apply, a settings issue) there is no
durable record to diagnose it from, and no way for a user to hand over diagnostic info in a
bug report.

We are adding an **"Enable debug logs" toggle** to Settings. When enabled, the app logs
everything useful for debugging across **both** frontend and backend into **one unified,
timestamped, rotating log file** the user can open and share. When disabled, only
warnings/errors are captured (so bug reports still have context), and the verbose
debug/trace detail is suppressed.

Decisions (confirmed with user):
- **Viewing:** file in OS app-log dir + an "Open logs" button in Settings (reveals the file
  in the file manager). No in-app viewer.
- **When toggle is OFF:** warnings + errors still written to the file; debug/trace only when ON.

## Approach

Use the official **`tauri-plugin-log`** plugin (Rust + `@tauri-apps/plugin-log`). It is the
only turnkey way to get frontend `log()` calls and backend logs into the **same file**, with
rotation and correct app-log-dir resolution — the codebase already leans on official plugins
(`opener`, `dialog`). Frontend `debug()/info()/...` calls `invoke('plugin:log|log')`, which
enters the Rust log pipeline and is written to all configured targets including the file.

**Runtime toggle mechanism (verified):** the plugin has no post-init `set_max_level`. The
idiomatic runtime hook is a `.filter(closure)` backed by a process-global `AtomicBool`. The
plugin is built at compile-time level `Trace` (everything *eligible*); the closure decides
per-record: `warn`/`error` always pass, everything else passes only when the AtomicBool is on.

## Backend changes (`src-tauri/`)

### NEW `src/logging.rs`
- `pub static DEBUG_LOGGING: AtomicBool = AtomicBool::new(false);`
- `pub fn set_debug(on: bool)` / `pub fn is_debug() -> bool` (Relaxed ordering).
- `pub fn plugin<R: tauri::Runtime>() -> TauriPlugin<R>` building:
  ```rust
  tauri_plugin_log::Builder::new()
      .level(log::LevelFilter::Trace)                    // everything eligible
      .level_for("hyper", log::LevelFilter::Warn)
      .level_for("reqwest", log::LevelFilter::Info)
      .max_file_size(5_000_000)                          // 5 MB
      .rotation_strategy(RotationStrategy::KeepSome(3))
      .targets([
          Target::new(TargetKind::Stdout),
          Target::new(TargetKind::LogDir { file_name: Some("lord-of-the-files".into()) }),
          Target::new(TargetKind::Webview),             // Rust->devtools (dev convenience)
      ])
      .filter(|meta| meta.level() <= log::Level::Warn || is_debug())  // warn/error always
      .build()
  ```
  Note: `.filter` receives `&log::Metadata` (level only — no message), which is all we need.

### `src/settings.rs`
- Add to `SettingsState`: `#[serde(default)] pub debug_logging: bool,`. The single-JSON-blob
  storage + `#[serde(default)]` + existing `Default` derive makes this backward-compatible
  (old rows deserialize to `false`). No other change — `load_state`/`save_state` handle it.
- Extend the `save_then_load_round_trips` test to assert `debug_logging` round-trips and that
  a blob missing the key defaults to `false`.

### `src/commands.rs`
- New command (register in `lib.rs` handler):
  ```rust
  #[tauri::command]
  pub fn set_debug_logging(db: State<SettingsDb>, enabled: bool) -> Result<(), String> {
      let conn = db.0.lock().map_err(|e| e.to_string())?;
      let mut state = settings::load_state(&conn);
      state.debug_logging = enabled;
      settings::save_state(&conn, &state)?;
      crate::logging::set_debug(enabled);
      log::info!("debug logging {}", if enabled { "enabled" } else { "disabled" });
      Ok(())
  }
  ```

### `src/lib.rs`
- `mod logging;`
- Register the log plugin **first** (before `opener`/`dialog`) so early logs are captured:
  `.plugin(logging::plugin())`.
- In `.setup()`, after opening `settings.db`: read persisted flag and prime the AtomicBool
  before `app.manage(...)` moves the connection:
  `let st = settings::load_state(&settings_conn); logging::set_debug(st.debug_logging);`
  then `log::info!("started; debug_logging={}", st.debug_logging);`
- Add `commands::set_debug_logging` to `generate_handler![]`.

### Backend instrumentation (what / where / level)
Follow existing thin-wrapper style; add log calls, don't restructure.
- **`ai.rs::generate`**: `info` at start (generation id, entry/chunk counts, profile id,
  `has_key`, base_url, model, chunk_size, concurrency, timeout, max_len) and on completion
  (result count, failed chunk tally); `warn` on total/partial failure; `trace` for instruction
  and system prompts; per-chunk `debug` (dispatch, timing, parse path, reconcile stats,
  old→new renames); `trace` for per-chunk user prompt and raw model response; `warn` on
  timeout, API errors (status code, no secrets), and JSON parse failure (truncated preview).
- **`commands.rs::ai_generate` / `test_connection`**: `info` on entry with generation id,
  counts, profile id/label, `has_key`, prompt length; `trace` for full prompt; `warn` on
  test_connection failure.
- **`commands.rs`** apply/undo/redo/scan: `info` on entry with counts, `warn!` per failure,
  `debug!` for old→new pairs.
- **`settings.rs`** keychain: `warn!` when keychain unavailable, `debug!("key set/cleared for {profile_id}")`.
- **REDACTION (critical):** never log the API key, `Authorization` header, or key value.
  Log `has_key`/`profile_id` only. This applies in `ai.rs`, `commands.rs`, `settings.rs`.

### Frontend AI instrumentation (`store.ts`)
- **`generateAi`**: `info` on start (step id, generation id, entry count, profile, model,
  `hasKey`, chunk/concurrency/timeout/maxLen config) and on success (result count, failed
  chunk tally); `warn` on partial failure; `debug` per-chunk progress events, per-file
  old→new renames (first 50), files with no suggestion, changed vs unchanged tally;
  `trace` for full user prompt and ignored stale progress events; `info` when superseding
  an in-flight generation; `debug` when skipping or discarding cancelled/stale generations.
- **`cancelAi`**: `info` with step id and generation id when known.
- **`testConnection`**: `info` on start/success, `warn` on failure.
- **`setActiveProfile` / `upsertProfile` / `deleteProfile` / `clearApiKey`**: `debug` on
  success (profile id only — never log keys).
- **`setDebugLogging`**: `info` when toggled.
- **`runPreview`**: `warn` per pipeline step error returned by the engine.
- **`applyAll` / undo-redo confirm**: `info` on completion with counts, `warn` per failure.
- **`addPaths`**: `debug` on scan, `error` on failure.
- **`GeneralSettings` open logs**: `error` on failure.

### `Cargo.toml`
- Add `tauri-plugin-log = "2"` and `log = "0.4"` (needed for the `log::*!` macros / `Metadata`).

### `capabilities/default.json`
- Add `"log:default"` to `permissions` (frontend `invoke('plugin:log|log')` is gated by it —
  without it frontend logging silently fails). `opener:default` already grants
  `reveal-item-in-dir`, so no opener change is needed.

## Frontend changes (`src/`)

### NEW `src/lib/log.ts`
Thin re-export wrapper (mirrors the `ipc.ts` convention, single import site, easy to no-op in
tests). REDACTION comment: never pass secrets to these.
```ts
import { trace, debug, info, warn, error } from "@tauri-apps/plugin-log";
export const log = { trace, debug, info, warn, error };
```

### `src/lib/ipc.ts`
- Add `export function setDebugLogging(enabled: boolean): Promise<void>` →
  `invoke("set_debug_logging", { enabled })`.

### `src/lib/types.ts`
- Add `debugLogging: boolean;` to `SettingsState`.

### `src/store.ts`
- Initial `settings` signal: add `debugLogging: false`.
- New action `setDebugLogging(enabled)` — calls `ipc.setDebugLogging`, then `loadSettings()`,
  `setNotice` on error (same pattern as `setActiveProfile`).
- Instrument existing try/catch actions: in each `catch (e)` add `log.error(...)` alongside the
  existing `setNotice(...)`; add `log.debug/info` at the top of key actions (`applyAll`,
  `runPreview`, `generateAi`, `undo`/`redo`, `addPaths`, settings actions).
  **REDACTION:** `saveApiKey` must log only `profileId` — never `key`.

### `src/App.tsx`
- In `onMount`: attach global `window.onerror` / `unhandledrejection` → `log.error(...)` so
  uncaught frontend errors are captured (these are error-level, so captured even when toggle off).
- Optional dev convenience: `attachConsole()` to mirror Rust logs into devtools.

### Settings UI — new "General" section
The panel currently only lists provider profiles; add a general section to the **list view**.
- **NEW `src/components/SettingsPanel/GeneralSettings.tsx`**:
  - A toggle bound to `s.settings().debugLogging`, calling `s.setDebugLogging(checked)`.
  - An "Open logs" button: `revealItemInDir((await appLogDir()) + "/lord-of-the-files.log")`
    using `@tauri-apps/plugin-opener` + `appLogDir` from `@tauri-apps/api/path`; wrap in
    try/catch → `setNotice`.
  - Help line: "Enable to capture detailed logs, then share the file for bug reports."
- **`src/components/SettingsPanel/ProfileList.tsx`**: render `<GeneralSettings />` above the
  provider list (it lives in the `list` branch of `index.tsx`).

### `package.json`
- Add `"@tauri-apps/plugin-log": "^2"`.

## Install / build notes
- `pnpm add @tauri-apps/plugin-log` (frontend). Cargo deps picked up on next `pnpm tauri dev`
  build; Tauri regenerates the ACL schema so `log:default` resolves.

## Verification
1. `cd src-tauri && cargo test` — settings round-trip incl. `debug_logging`; app compiles.
2. `pnpm tauri dev`. Open Settings → confirm the new General section with the toggle + "Open logs".
3. Toggle OFF: trigger an error (e.g. AI generate with no provider, or a failing apply).
   Open logs → file exists and contains the **error/warn** line, but no debug/trace noise.
4. Toggle ON: run a scan, a preview, an apply, and an AI generate. Open logs → file now
   contains debug/info detail from **both** frontend (store actions) and backend (commands/ai),
   interleaved by timestamp.
5. Grep the log file for the API key value → **must be absent** (redaction check). Confirm
   `has_key`/profile ids appear but no secret.
6. Restart the app with the toggle ON → confirm it persists (startup log line shows
   `debug_logging=true` and debug lines resume without re-toggling).
