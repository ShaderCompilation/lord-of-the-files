import { For, Show } from "solid-js";

import { PROVIDER_PRESETS } from "../../lib/providers";
import type { ProfileEditor } from "./profileEditor";

export function ProviderGrid(props: { editor: ProfileEditor }) {
  const e = props.editor;
  return (
    <div class="settings-step">
      <span class="settings-step-title">1 · Choose provider</span>
      <div class="provider-grid">
        <For each={PROVIDER_PRESETS}>
          {(preset) => (
            <button
              type="button"
              class="provider-tile"
              classList={{ selected: e.selectedProvider() === preset.label }}
              onClick={() => e.selectPreset(preset.label)}
            >
              {preset.label}
            </button>
          )}
        </For>
      </div>
      <Show when={e.isCustom()}>
        <label class="field">
          Base URL
          <input
            class="mono"
            placeholder="https://…/v1"
            value={e.draft()?.baseUrl ?? ""}
            onInput={(ev) => e.update({ baseUrl: ev.currentTarget.value })}
          />
        </label>
      </Show>
    </div>
  );
}
