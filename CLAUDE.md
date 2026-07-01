# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Lord of the Files: a Tauri v2 desktop app. Stage 1 (current, in progress) is a batch **file
renamer** with a composable transform pipeline, live diff preview, conflict detection, and
persistent undo/redo, plus a BYOK (bring-your-own-key) AI rename step.

- **Shell:** Tauri v2 · **Engine:** Rust (`src-tauri/`) · **UI:** SolidJS + TypeScript (`src/`)
- The Rust engine is the single source of truth: both the live preview and the actual rename
  derive new names from the same `compute_preview` code path — never reimplement rename logic
  in the frontend.

## Commands

```bash
pnpm install                    # install JS deps — use pnpm, not npm
pnpm tauri dev                  # run the app (Vite + Tauri)
pnpm exec tsc --noEmit          # frontend type check (no separate lint script)
pnpm build                      # vite build

cd src-tauri && cargo test              # engine, conflicts, history, full scan→apply→undo chain
cd src-tauri && cargo test some_name     # run a single test by name substring
cd src-tauri && cargo clippy --all-targets
```

BYOK dev/testing: open Settings in the app and add a provider profile (a preset prefills base
URL + default model). For offline dev, run [Ollama](https://ollama.com) locally and pick the
Ollama preset with a blank API key. Headless/CI can skip the OS keychain via the `LOTF_API_KEY`
env var.

## Architecture

### Rust backend (`src-tauri/src/`)

- `engine/{mod,steps,conflicts}.rs` — the pipeline transform engine and validation. This is the
  **single source of truth** for renaming: `compute_preview` runs a `Pipeline` of `StepConfig`s
  over `FileEntry`s to produce new names, and both live preview and the real apply call it.
  Steps mutate a `NameParts { stem, ext }` under a `Scope` (Stem/Ext/Full). Adding a new step
  type means: a variant in `types.rs::Step`, handling in `engine/steps.rs`, a `StepCard/*Fields.tsx`
  UI, and a `defaultStep` case in `src/lib/steps.ts`.
- `fs_scan.rs` — turns user-selected paths into a deduplicated `FileEntry` list (handles
  recursive descent).
- `history.rs` — SQLite-backed operation history with two-phase apply and multi-level undo/redo.
- `settings.rs` — non-secret provider config (`ProviderProfile`, `SettingsState`) persisted as a
  single JSON blob in SQLite (`settings.db`), separate from API keys.
- `ai.rs` — BYOK AI rename: builds an `aisdk`-backed OpenAI-compatible client per call
  (`base_url` + `model` + key), chunks `FileEntry` batches (`chunk_size`/`concurrency` per
  profile), dispatches with bounded concurrency, and lenient-parses the JSON response (handles
  bare arrays, fenced/prose-wrapped JSON). Deliberately never sends `response_format`/structured
  output, so incompatible providers can't reject the request — see
  `docs/byok-ai-rename-plan.md` for the full rationale. Unknown ids from the model are dropped;
  missing ids fall back to the original name in the engine.
- `logging.rs` — wraps `tauri-plugin-log` with a runtime `AtomicBool` toggle (`DEBUG_LOGGING`):
  warn/error always logged, debug/trace only when the Settings toggle is on. See `docs/logging.md`.
- `commands.rs` / `lib.rs` / `types.rs` — thin Tauri command handlers (real logic stays in the
  modules above), plugin/state wiring, and shared IPC types.

**Secrets:** API keys live in the OS keychain (`keyring` crate), never in SQLite, and never
cross back over IPC after entry — the frontend only ever learns `hasKey: bool`. Never log a key,
`Authorization` header, or other secret value; log `has_key`/`profile_id` only (this convention
is enforced throughout `ai.rs`/`commands.rs`/`settings.rs`).

### Frontend (`src/`)

- `store.ts` — the single reactive-state module (Solid signals/stores). Holds files, pipeline,
  preview, history, AI-generation state, and settings, plus all the actions that call into
  `lib/ipc.ts`. Deliberately centralized so the files → pipeline → preview reactive graph lives
  in one place; avoid introducing per-component local copies of this state.
- `lib/ipc.ts` — the only place that calls `invoke(...)`; one wrapper function per Tauri command.
- `lib/types.ts` — TypeScript mirrors of the Rust IPC types (keep camelCase in sync with the
  `#[serde(rename_all = "camelCase")]` Rust structs).
- `lib/steps.ts` / `lib/presets.ts` / `lib/providers.ts` — step defaults, saved pipeline presets
  (localStorage), and provider preset list (OpenAI, OpenRouter, Groq, Ollama, etc.).
- `lib/log.ts` — thin wrapper around `@tauri-apps/plugin-log`; use this instead of `console.*`
  so frontend logs land in the same unified log file as the Rust backend.
- `components/StepCard/` — one `*Fields.tsx` per step type, dispatched from `StepCard/index.tsx`.
- `components/SettingsPanel/` — provider profile CRUD UI (`ProfileList`/`ProfileForm`/
  `ProviderGrid`/`ApiKeyField`) plus `GeneralSettings.tsx` (debug logging toggle, open logs).
- `components/common/` — shared primitives (`Button`, `Checkbox`, `Field`, `Badge`, `Overlay`).

### Data flow

`addPaths` (dialog/drop) → `fs_scan::scan_paths` → `files` signal → pipeline edits bump
`pipelineVersion` → `runPreview` calls `engine::compute_preview` → diffed rows rendered in
`FileTable`/`DiffText` → `applyAll` calls `history::apply_rename` (two-phase, recorded for
undo/redo) → `refreshHistory`.

## Reference docs

- `docs/byok-ai-rename-plan.md` — full design for the BYOK AI rename feature (provider adapter
  choice, chunking/reconciliation, settings/keychain layout, prompt contract).
- `docs/logging.md` — design for the unified frontend+backend debug logging system.


##

- Don't try to check the UI visually(via screenshots, MCP or other tools) unless specifically instructed to.