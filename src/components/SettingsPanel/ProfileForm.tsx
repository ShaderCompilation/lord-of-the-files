import { Show } from "solid-js";

import { Button, TextField } from "../common";
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
            <TextField
              label="Model"
              mono
              placeholder="model id"
              value={d().model}
              onInput={(v) => e.update({ model: v })}
            />
            <TextField
              label="Nickname (optional)"
              placeholder={e.nickPlaceholder()}
              value={d().label}
              onInput={(v) => e.update({ label: v })}
            />
            <AdvancedSettings editor={e} />
          </div>

          <div class="settings-actions">
            <Button variant="primary" disabled={!e.canSave()} onClick={onSave}>
              Save
            </Button>
            <Button variant="ghost" disabled={!e.canSave() || e.testing()} onClick={e.test}>
              {e.testing() ? "Testing…" : "Test connection"}
            </Button>
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
