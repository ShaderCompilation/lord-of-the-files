import { For, Show } from "solid-js";

import * as s from "../../store";
import { Button, SelectField, TextareaField } from "../common";
import type { SetFn, Variant } from "./types";

export function AiFields(props: { step: Variant<"ai">; set: SetFn; id: string }) {
  const running = () => s.isAiLoading(props.id);
  const progress = () => s.aiProgressFor(props.id);
  const error = () => s.aiErrorFor(props.id);
  const pct = () => {
    const p = progress();
    return p && p.totalChunks > 0 ? Math.round((p.completedChunks / p.totalChunks) * 100) : 0;
  };

  return (
    <>
      <TextareaField
        label="Prompt"
        value={props.step.prompt}
        onInput={(v) => props.set({ prompt: v })}
        placeholder="e.g. Make names Title Case and human-readable"
      />

      <Show when={s.settings().profiles.length > 0}>
        <SelectField
          label="Provider"
          value={s.activeProfile()?.id ?? ""}
          onChange={(v) => s.setActiveProfile(v)}
        >
          <For each={s.settings().profiles}>
            {(profile) => (
              <option value={profile.id}>
                {profile.label} ({profile.model})
              </option>
            )}
          </For>
        </SelectField>
      </Show>

      <div class="ai-row">
        <Show
          when={!running()}
          fallback={
            <Button variant="ghost" onClick={() => s.cancelAi(props.id)}>
              Cancel
            </Button>
          }
        >
          <Button
            onClick={() => s.generateAi(props.id, props.step.prompt)}
            disabled={s.files().length === 0 || !props.step.prompt.trim() || !s.activeProfile()}
          >
            Generate
          </Button>
        </Show>

        <Show
          when={running()}
          fallback={
            <span class="muted">
              {props.step.results ? `${props.step.results.length} suggestion(s) cached` : "not run yet"}
            </span>
          }
        >
          <div class="ai-progress">
            <div class="ai-progress-bar">
              <div class="ai-progress-fill" style={{ width: `${pct()}%` }} />
            </div>
            <span class="muted small">
              batch {progress()?.completedChunks ?? 0} / {progress()?.totalChunks || "?"}
              {" · "}
              {progress()?.suggestedSoFar ?? 0} suggested
            </span>
          </div>
        </Show>
      </div>

      <Show when={running() && progress()?.lastChunkError}>
        <p class="muted small ai-progress-error">Latest batch error: {progress()!.lastChunkError}</p>
      </Show>

      <Show when={error()}>
        <p class="step-error">{error()}</p>
      </Show>

      <Show when={!s.activeProfile()}>
        <p class="muted small">Set up a provider in Settings to use AI Rename.</p>
      </Show>
    </>
  );
}
