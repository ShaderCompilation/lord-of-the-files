import { Show } from "solid-js";

import { AdvancedSettings } from "./AdvancedSettings";
import { ApiKeyField } from "./ApiKeyField";
import type { ProfileEditor } from "./profileEditor";
import { ProviderGrid } from "./ProviderGrid";

export function ProfileForm(props: { editor: ProfileEditor; onDone: () => void }) {
  const e = props.editor;

  async function onSave() {
    if (await e.save()) props.onDone();
  }

  return (
    <Show when={e.draft()}>
      {(d) => (
        <>
          <ProviderGrid editor={e} />
          <ApiKeyField editor={e} />

          <div class="settings-step">
            <span class="settings-step-title">3 · Model &amp; details</span>
            <label class="field">
              Model
              <input
                class="mono"
                placeholder="model id"
                value={d().model}
                onInput={(ev) => e.update({ model: ev.currentTarget.value })}
              />
            </label>
            <label class="field">
              Nickname (optional)
              <input
                placeholder={e.nickPlaceholder()}
                value={d().label}
                onInput={(ev) => e.update({ label: ev.currentTarget.value })}
              />
            </label>
            <AdvancedSettings editor={e} />
          </div>

          <div class="settings-actions">
            <button type="button" class="primary" disabled={!e.canSave()} onClick={onSave}>
              Save
            </button>
            <button
              type="button"
              class="ghost"
              disabled={!e.canSave() || e.testing()}
              onClick={e.test}
            >
              {e.testing() ? "Testing…" : "Test connection"}
            </button>
            <Show when={e.testStatus()}>
              {(status) => (
                <span classList={{ "test-ok": status().ok, "test-error": !status().ok }}>
                  {status().ok ? "✔ ok" : `✕ ${status().message}`}
                </span>
              )}
            </Show>
          </div>
        </>
      )}
    </Show>
  );
}
