import { For, Show, createSignal } from "solid-js";

import * as s from "../store";
import {
  STEP_DESCRIPTIONS,
  STEP_GROUPS,
  STEP_ICONS,
  STEP_LABELS,
} from "../lib/steps";
import type { StepType } from "../lib/types";
import { Button } from "./common";

/** "+ Add step" trigger that opens a grouped menu of step types with descriptions. */
export function AddStepMenu(props: { label?: string; class?: string }) {
  const [open, setOpen] = createSignal(false);

  const pick = (type: StepType) => {
    s.addStep(type);
    setOpen(false);
  };

  return (
    <div class="addstep">
      <Button
        class={props.class ?? "addstep-trigger"}
        onClick={() => setOpen((v) => !v)}
      >
        {props.label ?? "+ Add step"}
      </Button>

      <Show when={open()}>
        <div class="addstep-menu" role="menu">
          <For each={STEP_GROUPS}>
            {(group) => (
              <div class="addstep-group" classList={{ ai: group.ai }}>
                <div class="addstep-group-label">{group.label}</div>
                <For each={group.types}>
                  {(type) => (
                    <button
                      type="button"
                      role="menuitem"
                      class="addstep-item"
                      classList={{ ai: type === "ai" }}
                      onClick={() => pick(type)}
                    >
                      <span class="step-icon" classList={{ ai: type === "ai" }}>
                        {STEP_ICONS[type]}
                      </span>
                      <span class="addstep-item-text">
                        <span class="addstep-item-title">
                          {STEP_LABELS[type]}
                        </span>
                        <span class="addstep-item-desc">
                          {STEP_DESCRIPTIONS[type]}
                        </span>
                      </span>
                    </button>
                  )}
                </For>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
