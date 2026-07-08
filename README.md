# Lord of the Files

A desktop suite of file tools. **Stage 1** is a batch **file renamer** with a composable
transform pipeline, live two-column diff preview, conflict detection, and persistent
undo/redo.

- **Shell:** Tauri v2 · **Engine:** Rust · **UI:** SolidJS + TypeScript
- The Rust engine is the single source of truth: both the live preview and the actual
  rename derive new names from the same `compute_preview` code path.

## Features (Stage 1)

- Add files/folders via dialog or drag-and-drop. Recursive descent and extension
  preservation are on by default (both toggleable).
- **Composable pipeline** of ordered, reorderable, toggleable steps:
  Find & Replace · Regex · Change Case · Insert · Remove · Clean Up · Counter · AI Rename.
- Two-column preview with inline char-level diff (green = added, red = removed) and
  per-row status: Unchanged / Changed / Conflict / Invalid.
- Conflict & validation detection (collisions, pre-existing files, invalid chars,
  case-only changes) blocks unsafe applies.
- Known v1 caveat: Unicode-equivalent names with different normalization forms (for example
  NFC vs NFD) are not currently folded together, so some macOS-style normalization collisions
  may not be caught before apply.
- **Persistent history** with multi-level undo/redo (SQLite in the app data dir).
- Pipeline **presets** saved in localStorage.
- **AI Rename** step is BYOK (bring your own key): configure any OpenAI-compatible provider
  in Settings — OpenAI, OpenRouter, Groq, Together, Fireworks, DeepInfra, Mistral, DeepSeek,
  xAI, Perplexity, Gemini, or local Ollama/LM Studio. Keys are stored in the OS keychain and
  never leave the Rust side after entry.

## Prerequisites

- Node + [pnpm](https://pnpm.io), Rust toolchain, and the Tauri Linux system deps
  (`webkit2gtk-4.1`).

## Develop

```bash
pnpm install

# Run the app (Vite + Tauri)
pnpm tauri dev
```

BYOK: open Settings in the app and add a provider profile (a preset prefills the base URL and
a default model). For offline dev, run [Ollama](https://ollama.com) locally and pick the
Ollama preset with a blank API key. Headless/CI can skip the keychain via `LOTF_API_KEY`.

## Test

```bash
cd src-tauri && cargo test     # engine, conflicts, history, full scan→apply→undo chain
pnpm exec tsc --noEmit         # frontend type check
```

## Build

```bash
pnpm tauri build
```

Produces native installers/bundles for the host OS (`bundle.targets` is `"all"` in
`src-tauri/tauri.conf.json`) — e.g. `.deb`/`.rpm`/AppImage on Linux, `.app`/`.dmg` on macOS,
NSIS `.exe`/`.msi` on Windows. **Builds are currently unsigned** — no code-signing identity is
configured, so macOS Gatekeeper and Windows SmartScreen will show an "unknown publisher"
warning on first launch. See the [Tauri distribution docs](https://v2.tauri.app/distribute/) if
signing is added later.

## Layout

```
src-tauri/src/
  engine/{mod,steps,conflicts}.rs   # pipeline transforms + validation (single source of truth)
  fs_scan.rs                        # selection -> FileEntry list (recursive, dedup)
  history.rs                        # SQLite, two-phase apply, undo/redo
  settings.rs                       # provider profiles (SQLite) + API keys (OS keychain)
  ai.rs                             # BYOK: aisdk-backed OpenAI-compatible adapter, chunking
  types.rs / commands.rs / lib.rs   # shared types, Tauri commands, wiring
src/
  store.ts                          # central reactive state + actions
  lib/{types,ipc,diff,steps,presets,providers}.ts
  components/{Toolbar,FileTable,DiffText,StepCard,PipelineEditor,HistoryPanel,SettingsPanel}.tsx
```

## BYOK AI rename

The AI step sends filenames (stem + extension + parent folder hint) and your instruction to
whichever OpenAI-compatible endpoint is configured as the active provider in Settings. No
content is ever read from the files themselves. Requests are chunked and dispatched with
bounded concurrency (both configurable per profile); a chunk that errors or times out is
counted as a partial failure rather than aborting the whole batch.
