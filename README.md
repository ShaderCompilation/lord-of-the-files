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
- **Persistent history** with multi-level undo/redo (SQLite in the app data dir).
- Pipeline **presets** saved in localStorage.
- **AI Rename** step calls a backend over a versioned contract; a local mock backend is
  included for offline dev.

## Prerequisites

- Node + [pnpm](https://pnpm.io), Rust toolchain, and the Tauri Linux system deps
  (`webkit2gtk-4.1`).

## Develop

```bash
pnpm install

# Optional: run the mock AI backend (needed only for the AI Rename step)
pnpm mock-backend           # serves http://localhost:8787/v1/rename

# Run the app (Vite + Tauri)
pnpm tauri dev
```

The AI step's backend URL defaults to the mock above and can be overridden:

```bash
LOTF_BACKEND_URL=https://my-backend/v1/rename pnpm tauri dev
```

## Test

```bash
cd src-tauri && cargo test     # engine, conflicts, history, full scan→apply→undo chain
pnpm exec tsc --noEmit         # frontend type check
```

## Layout

```
src-tauri/src/
  engine/{mod,steps,conflicts}.rs   # pipeline transforms + validation (single source of truth)
  fs_scan.rs                        # selection -> FileEntry list (recursive, dedup)
  history.rs                        # SQLite, two-phase apply, undo/redo
  ai.rs                             # AI backend client + contract
  types.rs / commands.rs / lib.rs   # shared types, Tauri commands, wiring
src/
  store.ts                          # central reactive state + actions
  lib/{types,ipc,diff,steps,presets}.ts
  components/{Toolbar,FileTable,DiffText,StepCard,PipelineEditor,HistoryPanel}.tsx
mock-backend/server.mjs             # dev stand-in for the AI backend
```

## AI backend contract

`POST {LOTF_BACKEND_URL}` — see `src/lib/types.ts` (`AiRequestFile` / `AiResponse`) and
`mock-backend/server.mjs`. Filenames are sent without extension (extensions are preserved
client-side); the backend returns `{ id, newName }` per file.
