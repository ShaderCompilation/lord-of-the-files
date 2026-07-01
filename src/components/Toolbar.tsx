import { open } from "@tauri-apps/plugin-dialog";
import { createMemo } from "solid-js";

import * as s from "../store";
import { Button, Checkbox } from "./common";

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
        <Button onClick={addFiles}>Add files</Button>
        <Button onClick={addFolder}>Add folder</Button>
        <Button variant="ghost" onClick={s.clearFiles} disabled={s.files().length === 0}>
          Clear
        </Button>
      </div>

      <div class="toolbar-group">
        <Checkbox checked={s.recursive()} onChange={s.setRecursive}>
          Recursive
        </Checkbox>
        <Checkbox checked={s.preserveExt()} onChange={s.setPreserveExt}>
          Preserve extension
        </Checkbox>
        <Checkbox checked={s.includeDirs()} onChange={s.setIncludeDirs}>
          Include folders
        </Checkbox>
      </div>

      <div class="toolbar-spacer" />

      <div class="toolbar-group">
        <span class="muted">
          {stats().changed} to rename
          {stats().blocking > 0 ? ` · ${stats().blocking} blocked` : ""}
        </span>
        <Button variant="ghost" onClick={props.onToggleSettings}>
          Settings
        </Button>
        <Button variant="ghost" onClick={props.onToggleHistory}>
          History
        </Button>
        <Button
          variant="primary"
          onClick={s.applyAll}
          disabled={stats().changed === 0 || stats().blocking > 0}
          title={stats().blocking > 0 ? "Resolve conflicts before applying" : ""}
        >
          Apply rename
        </Button>
      </div>
    </header>
  );
}
