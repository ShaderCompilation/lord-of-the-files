import { For, Show } from "solid-js";

import { PROVIDER_PRESETS } from "../../lib/providers";
import { Button, TextField } from "../common";
import type { ProfileEditor } from "./profileEditor";

export function ProviderGrid(props: { editor: ProfileEditor }) {
  const e = props.editor;
  return (
    <div class="settings-step">
      <span class="settings-step-title">1 · Choose provider</span>
      <div class="provider-grid">
        <For each={PROVIDER_PRESETS}>
          {(preset) => (
            <Button
              class={`provider-tile${e.selectedProvider() === preset.label ? " selected" : ""}`}
              onClick={() => e.selectPreset(preset.label)}
            >
              {preset.label}
            </Button>
          )}
        </For>
      </div>
      <Show when={e.isCustom()}>
        <TextField
          label="Base URL"
          mono
          placeholder="https://…/v1"
          value={e.draft()?.baseUrl ?? ""}
          onInput={(v) => e.update({ baseUrl: v })}
        />
      </Show>
    </div>
  );
}
