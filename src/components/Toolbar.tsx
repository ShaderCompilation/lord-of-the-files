import { Show, createMemo } from "solid-js";

import * as s from "../store";
import { Button, Checkbox } from "./common";

export function Toolbar(props: {
  onToggleHistory: () => void;
  onToggleSettings: () => void;
  onToggleDevMenu: () => void;
}) {
  const hasFiles = () => s.files().length > 0;
  const applicable = createMemo(() => s.applicableRows().length);
  const conflicts = createMemo(() => s.previewCounts().conflict);
  const mockAiOn = () => import.meta.env.DEV && s.settings().mockAi.enabled;

  const jumpToConflicts = () => s.setTableFilter("conflict");

  return (
    <header class="toolbar">
      <div class="brand">Lord of the Files</div>

      <div class="toolbar-group">
        <Button variant="primary" onClick={s.pickFiles}>
          Add files
        </Button>
        <Button variant="primary" onClick={() => void s.pickFolder()}>
          Add folder…
        </Button>
        <Checkbox checked={s.recursive()} onChange={s.setRecursive}>
          Recursive
        </Checkbox>
        <Checkbox checked={s.includeDirs()} onChange={s.setIncludeDirs}>
          Include folders
        </Checkbox>
      </div>

      <Show when={hasFiles()}>
        <Button variant="ghost" onClick={s.clearFiles}>
          Clear
        </Button>
      </Show>

      <div class="toolbar-spacer" />

      <Show when={mockAiOn()}>
        <Button
          class="mock-ai-pill"
          onClick={props.onToggleDevMenu}
          title="AI renames are simulated — open the Dev menu to change this"
        >
          🧪 Mock AI
        </Button>
      </Show>

      <Show when={import.meta.env.DEV}>
        <Button variant="ghost" onClick={props.onToggleDevMenu}>
          Dev
        </Button>
      </Show>
      <Button variant="ghost" onClick={props.onToggleSettings}>
        Settings
      </Button>
      <Button variant="ghost" onClick={props.onToggleHistory}>
        History
      </Button>

      <Show when={conflicts() > 0}>
        <Button
          class="conflict-pill"
          onClick={jumpToConflicts}
          title="Show conflicting rows"
        >
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
