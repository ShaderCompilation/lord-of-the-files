import { Show } from "solid-js";

import type { ProfileEditor } from "./profileEditor";

export function AdvancedSettings(props: { editor: ProfileEditor }) {
  const e = props.editor;
  return (
    <>
      <button type="button" class="ghost small" onClick={e.toggleAdvanced}>
        {e.advancedOpen() ? "Hide advanced" : "Show advanced"}
      </button>
      <Show when={e.advancedOpen()}>
        <Show when={!e.isCustom()}>
          <label class="field">
            Base URL
            <input
              class="mono"
              value={e.draft()?.baseUrl ?? ""}
              onInput={(ev) => e.update({ baseUrl: ev.currentTarget.value })}
            />
          </label>
        </Show>
        <label class="field">
          Chunk size
          <input
            type="number"
            min="1"
            value={e.draft()?.chunkSize ?? 0}
            onInput={(ev) => e.update({ chunkSize: Number(ev.currentTarget.value) })}
          />
        </label>
        <label class="field">
          Concurrency
          <input
            type="number"
            min="1"
            value={e.draft()?.concurrency ?? 0}
            onInput={(ev) => e.update({ concurrency: Number(ev.currentTarget.value) })}
          />
        </label>
        <label class="field">
          Max name length
          <input
            type="number"
            min="1"
            value={e.draft()?.maxLen ?? 0}
            onInput={(ev) => e.update({ maxLen: Number(ev.currentTarget.value) })}
          />
        </label>
        <label class="field">
          Timeout (seconds)
          <input
            type="number"
            min="1"
            value={e.draft()?.timeoutSecs ?? 0}
            onInput={(ev) => e.update({ timeoutSecs: Number(ev.currentTarget.value) })}
          />
        </label>
      </Show>
    </>
  );
}
