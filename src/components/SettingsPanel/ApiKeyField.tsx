import { Show } from "solid-js";

import type { ProfileEditor } from "./profileEditor";

export function ApiKeyField(props: { editor: ProfileEditor }) {
  const e = props.editor;
  return (
    <div class="settings-step">
      <span class="settings-step-title">2 · API key</span>
      <Show
        when={e.draft()?.hasKey && !e.replacingKey()}
        fallback={
          <input
            type="password"
            placeholder="Paste your API key"
            value={e.keyInput()}
            onInput={(ev) => e.setKeyInput(ev.currentTarget.value)}
          />
        }
      >
        <div class="key-saved">
          <span class="badge badge-changed">✔ Key saved</span>
          <button type="button" class="ghost small" onClick={() => e.setReplacingKey(true)}>
            Replace
          </button>
          <button type="button" class="ghost small" onClick={e.removeKey}>
            Remove
          </button>
        </div>
      </Show>
      <Show when={e.keyHint()}>{(hint) => <p class="muted small">{hint()}</p>}</Show>
    </div>
  );
}
