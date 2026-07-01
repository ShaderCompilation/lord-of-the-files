import { Show } from "solid-js";

import * as s from "../../store";
import { Button, TextareaField } from "../common";
import type { SetFn, Variant } from "./types";

export function AiFields(props: { step: Variant<"ai">; set: SetFn; id: string }) {
  return (
    <>
      <TextareaField
        label="Prompt"
        value={props.step.prompt}
        onInput={(v) => props.set({ prompt: v })}
        placeholder="e.g. Make names Title Case and human-readable"
      />
      <div class="ai-row">
        <Button
          onClick={() => s.generateAi(props.id, props.step.prompt)}
          disabled={
            s.isAiLoading(props.id) ||
            s.files().length === 0 ||
            !props.step.prompt.trim() ||
            !s.activeProfile()
          }
        >
          {s.isAiLoading(props.id) ? "Generating…" : "Generate"}
        </Button>
        <span class="muted">
          {props.step.results ? `${props.step.results.length} suggestion(s) cached` : "not run yet"}
        </span>
      </div>
      <Show when={!s.activeProfile()}>
        <p class="muted small">Set up a provider in Settings to use AI Rename.</p>
      </Show>
    </>
  );
}
