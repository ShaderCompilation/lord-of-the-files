import { Show, createMemo, createSignal } from "solid-js";

import * as s from "../store";
import { Button, Checkbox } from "./common";

export function Toolbar(props: { onToggleHistory: () => void; onToggleSettings: () => void }) {
  const [addOpen, setAddOpen] = createSignal(false);

  const hasFiles = () => s.files().length > 0;
  const applicable = createMemo(() => s.applicableRows().length);
  const conflicts = createMemo(() => s.previewCounts().conflict);

  const jumpToConflicts = () => s.setTableFilter("conflict");

  return (
    <header class="toolbar">
      <div class="brand">🗂️ Lord of the Files</div>

      <div class="add-control">
        <Button variant="primary" class="add-main" onClick={s.pickFiles}>
          Add files
        </Button>
        <Button
          variant="primary"
          class="add-caret"
          onClick={() => setAddOpen((v) => !v)}
          title="More add options"
        >
          ▾
        </Button>
        <Show when={addOpen()}>
          <button class="menu-backdrop" aria-label="Close menu" onClick={() => setAddOpen(false)} />
          <div class="add-menu">
            <Button
              variant="ghost"
              class="add-menu-item"
              onClick={() => {
                setAddOpen(false);
                void s.pickFolder();
              }}
            >
              Add folder…
            </Button>
            <div class="add-menu-sep" />
            <div class="add-menu-label">Scan options</div>
            <Checkbox checked={s.recursive()} onChange={s.setRecursive}>
              Recursive
            </Checkbox>
            <Checkbox checked={s.includeDirs()} onChange={s.setIncludeDirs}>
              Include folders
            </Checkbox>
          </div>
        </Show>
      </div>

      <Button variant="ghost" onClick={s.clearFiles} disabled={!hasFiles()}>
        Clear
      </Button>

      <div class="toolbar-spacer" />

      <Button variant="ghost" onClick={props.onToggleSettings}>
        Settings
      </Button>
      <Button variant="ghost" onClick={props.onToggleHistory}>
        History
      </Button>

      <Show when={conflicts() > 0}>
        <Button class="conflict-pill" onClick={jumpToConflicts} title="Show conflicting rows">
          ⚠ {conflicts()} conflict{conflicts() === 1 ? "" : "s"}
        </Button>
      </Show>

      <Button
        variant="primary"
        onClick={s.applyAll}
        disabled={applicable() === 0 || conflicts() > 0}
        title={conflicts() > 0 ? "Resolve conflicts before applying" : ""}
      >
        {applicable() > 0
          ? `Rename ${applicable()} file${applicable() === 1 ? "" : "s"}`
          : "Rename"}
      </Button>
    </header>
  );
}
