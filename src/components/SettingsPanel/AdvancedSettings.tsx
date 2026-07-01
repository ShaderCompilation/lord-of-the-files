import { Show } from "solid-js";

import { Button, TextField } from "../common";
import type { ProfileEditor } from "./profileEditor";

export function AdvancedSettings(props: { editor: ProfileEditor }) {
  const e = props.editor;
  return (
    <>
      <Button variant="ghost" small onClick={e.toggleAdvanced}>
        {e.advancedOpen() ? "Hide advanced" : "Show advanced"}
      </Button>
      <Show when={e.advancedOpen()}>
        <Show when={!e.isCustom()}>
          <TextField
            label="Base URL"
            mono
            value={e.draft()?.baseUrl ?? ""}
            onInput={(v) => e.update({ baseUrl: v })}
          />
        </Show>
        <TextField
          label="Chunk size"
          type="number"
          min="1"
          value={e.draft()?.chunkSize ?? 0}
          onInput={(v) => e.update({ chunkSize: Number(v) })}
        />
        <TextField
          label="Concurrency"
          type="number"
          min="1"
          value={e.draft()?.concurrency ?? 0}
          onInput={(v) => e.update({ concurrency: Number(v) })}
        />
        <TextField
          label="Max name length"
          type="number"
          min="1"
          value={e.draft()?.maxLen ?? 0}
          onInput={(v) => e.update({ maxLen: Number(v) })}
        />
        <TextField
          label="Timeout (seconds)"
          type="number"
          min="1"
          value={e.draft()?.timeoutSecs ?? 0}
          onInput={(v) => e.update({ timeoutSecs: Number(v) })}
        />
      </Show>
    </>
  );
}
