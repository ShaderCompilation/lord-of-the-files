# Lord of the Files

[![CI](https://github.com/ShaderCompilation/lord-of-the-files/actions/workflows/ci.yml/badge.svg)](https://github.com/ShaderCompilation/lord-of-the-files/actions/workflows/ci.yml)
[![Desktop Builds](https://github.com/ShaderCompilation/lord-of-the-files/actions/workflows/desktop-builds.yml/badge.svg)](https://github.com/ShaderCompilation/lord-of-the-files/actions/workflows/desktop-builds.yml)

Lord of the Files is a desktop batch-renaming app for files and folders. It gives you a
composable rename pipeline, a live before/after diff, conflict detection, persistent
undo/redo, and an optional bring-your-own-key AI rename step.

The current release is focused on batch renaming. Future releases may add more file tools.

## Download

Installers are published on the
[GitHub Releases page](https://github.com/ShaderCompilation/lord-of-the-files/releases).

- **Linux:** download the AppImage, make it executable, then run it.
- **macOS:** download the universal DMG and drag the app into Applications.
- **Windows:** download the generated `.exe` or `.msi` installer.

Release builds are currently unsigned. macOS Gatekeeper and Windows SmartScreen may show an
unknown-publisher warning on first launch.

## Features

- Add files or folders with a file picker or drag-and-drop.
- Rename recursively, with extension preservation enabled by default.
- Build ordered pipelines from reusable steps:
  - Find & Replace
  - Regex
  - Change Case
  - Insert
  - Remove
  - Clean Up
  - Counter
  - AI Rename
- Preview every change in a two-column table with inline character-level diffs.
- Detect unsafe changes before applying: duplicate output names, existing destination files,
  invalid names, and case-only changes.
- Save pipeline presets locally.
- Undo and redo previous rename operations across app restarts.
- Review AI requests in a persistent AI history panel.

## Basic Usage

1. Add files or folders.
2. Build a rename pipeline from the step menu.
3. Review the preview table. Rows marked as conflicts or invalid must be fixed before apply.
4. Apply the rename.
5. Use History to inspect, undo, or redo past operations.

Lord of the Files only renames filesystem entries. It does not read or modify file contents.
For important folders, keep a backup until you are comfortable with the preview and history
flow.

## AI Rename

AI Rename is optional and bring-your-own-key. Open Settings, add a provider profile, choose a
preset, and enter your API key. Supported presets include OpenAI, OpenRouter, Groq, Together,
Fireworks, DeepInfra, Mistral, DeepSeek, xAI, Perplexity, Gemini, Ollama, and LM Studio.

When AI Rename is used, the app sends filenames, extensions, parent-folder hints, and your
instruction to the active provider. It does not send file contents. API keys are stored in the
OS keychain and are not sent back to the frontend after entry.

For local/offline workflows, run Ollama or LM Studio and select the matching provider preset.

## Known Limitations

- Release builds are not code-signed yet.
- Unicode-equivalent names with different normalization forms, such as NFC vs NFD, are not
  folded together before apply. This can miss some macOS-style normalization collisions.
- AI Rename quality depends on the configured provider, model, prompt, and available context.

## Architecture

- **Shell:** Tauri v2
- **Backend:** Rust
- **Frontend:** SolidJS and TypeScript
- **Storage:** SQLite for rename history, settings, and AI history; OS keychain for API keys

The Rust engine is the single source of truth for renaming. The preview and the actual rename
operation both derive output names from the same `compute_preview` path, so the frontend does
not reimplement rename logic.

## Development

### Prerequisites

- Node.js 20 or newer
- pnpm 11.8.0 or newer
- Rust stable
- Tauri v2 system dependencies for your platform

On Linux, Tauri requires WebKitGTK and related native packages. For Debian/Ubuntu-based
systems, install the packages listed in `.github/workflows/ci.yml`.

### Run Locally

```bash
pnpm install
pnpm tauri dev
```

Headless or CI environments can provide an AI key through `LOTF_API_KEY` when the OS keychain
is unavailable.

### Test

```bash
pnpm exec tsc --noEmit
pnpm test:frontend
cargo test --manifest-path src-tauri/Cargo.toml
```

### Build

```bash
pnpm tauri build
```

Tauri writes native bundles under `src-tauri/target/release/bundle/`.

## Release Process

GitHub Actions builds desktop installers from
[`.github/workflows/desktop-builds.yml`](.github/workflows/desktop-builds.yml).

To run a manual test build, use **Actions -> Desktop Builds -> Run workflow** in GitHub.

To publish a release:

```bash
# Keep these versions in sync first:
# - package.json
# - src-tauri/tauri.conf.json
# - src-tauri/Cargo.toml
git tag v0.1.0
git push origin v0.1.0
```

Pushing a `v*` tag runs Linux, macOS, and Windows builds. The workflow checks that the tag
matches the Tauri app version, uploads build artifacts, and publishes a GitHub Release with
generated release notes.

## Project Layout

```text
src-tauri/src/
  engine/                 Rename pipeline transforms and validation
  fs_scan.rs              Selection scanning, recursive descent, deduplication
  history.rs              SQLite-backed apply, undo, and redo
  settings.rs             Provider profiles and keychain-backed API keys
  ai.rs                   OpenAI-compatible AI rename adapter
  ai_history.rs           Persistent AI request history
  commands.rs             Tauri command handlers

src/
  store.ts                Central SolidJS state and app actions
  lib/                    IPC wrappers, shared types, diffs, presets, providers
  components/             Toolbar, file table, pipeline editor, settings, history
```

## License

MIT
