import { open } from "@tauri-apps/plugin-dialog";
import { createMemo } from "solid-js";

import * as s from "../store";

export function Toolbar(props: { onToggleHistory: () => void; onToggleSettings: () => void }) {
  const addFiles = async () => {
    const sel = await open({ multiple: true });
    if (sel) await s.addPaths(Array.isArray(sel) ? sel : [sel]);
  };
  const addFolder = async () => {
    const sel = await open({ directory: true, multiple: true });
    if (sel) await s.addPaths(Array.isArray(sel) ? sel : [sel]);
  };

  const stats = createMemo(() => {
    const rows = s.preview().rows;
    let changed = 0;
    let blocking = 0;
    for (const r of rows) {
      if (s.isExcluded(r.id)) continue;
      if (r.status === "changed") changed++;
      else if (r.status === "conflict" || r.status === "invalid") blocking++;
    }
    return { changed, blocking };
  });

  return (
    <header class="toolbar">
      <div class="brand">🗂️ Lord of the Files</div>

      <div class="toolbar-group">
        <button type="button" onClick={addFiles}>Add files</button>
        <button type="button" onClick={addFolder}>Add folder</button>
        <button type="button" class="ghost" onClick={s.clearFiles} disabled={s.files().length === 0}>
          Clear
        </button>
      </div>

      <div class="toolbar-group">
        <label class="check">
          <input
            type="checkbox"
            checked={s.recursive()}
            onChange={(e) => s.setRecursive(e.currentTarget.checked)}
          />
          Recursive
        </label>
        <label class="check">
          <input
            type="checkbox"
            checked={s.preserveExt()}
            onChange={(e) => s.setPreserveExt(e.currentTarget.checked)}
          />
          Preserve extension
        </label>
        <label class="check">
          <input
            type="checkbox"
            checked={s.includeDirs()}
            onChange={(e) => s.setIncludeDirs(e.currentTarget.checked)}
          />
          Include folders
        </label>
      </div>

      <div class="toolbar-spacer" />

      <div class="toolbar-group">
        <span class="muted">
          {stats().changed} to rename
          {stats().blocking > 0 ? ` · ${stats().blocking} blocked` : ""}
        </span>
        <button type="button" class="ghost" onClick={props.onToggleSettings}>
          Settings
        </button>
        <button type="button" class="ghost" onClick={props.onToggleHistory}>
          History
        </button>
        <button type="button"
          class="primary"
          onClick={s.applyAll}
          disabled={stats().changed === 0 || stats().blocking > 0}
          title={stats().blocking > 0 ? "Resolve conflicts before applying" : ""}
        >
          Apply rename
        </button>
      </div>
    </header>
  );
}
