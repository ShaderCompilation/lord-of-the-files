import { Show } from "solid-js";

import * as s from "../../store";
import { STEP_ICONS, STEP_LABELS } from "../../lib/steps";
import type { StepConfig } from "../../lib/types";
import { Button } from "../common";

export function StepCardHead(props: {
  step: StepConfig;
  index: number;
  collapsed: boolean;
  summary: string;
  onToggleCollapse: () => void;
  onDragStart?: () => void;
  onDragEnd?: () => void;
}) {
  const id = () => props.step.id;
  const isAi = () => props.step.type === "ai";

  return (
    <div class="stepcard-head">
      <span
        class="drag-handle"
        draggable={true}
        title="Drag to reorder"
        onDragStart={(e) => {
          e.dataTransfer?.setData("text/plain", String(props.index));
          if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
          props.onDragStart?.();
        }}
        onDragEnd={() => props.onDragEnd?.()}
      >
        ⠿
      </span>

      <span class="step-num">{props.index + 1}</span>

      <input
        type="checkbox"
        checked={props.step.enabled}
        onChange={() => s.toggleStep(id())}
        title="Enable / disable step"
      />

      <button type="button" class="stepcard-toggle" onClick={props.onToggleCollapse}>
        <span class="step-caret">{props.collapsed ? "▸" : "▾"}</span>
        <span class="step-icon" classList={{ ai: isAi() }}>
          {STEP_ICONS[props.step.type]}
        </span>
        <span class="stepcard-title">{STEP_LABELS[props.step.type]}</span>
        <Show when={props.collapsed}>
          <span class="stepcard-summary">{props.summary}</span>
        </Show>
      </button>

      <div class="stepcard-actions">
        <Button variant="icon" danger onClick={() => s.removeStep(id())} title="Remove">
          ✕
        </Button>
      </div>
    </div>
  );
}
