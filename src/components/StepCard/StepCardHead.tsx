import * as s from "../../store";
import { STEP_LABELS } from "../../lib/steps";
import type { StepConfig } from "../../lib/types";
import { Button } from "../common";

export function StepCardHead(props: { step: StepConfig; index: number; total: number }) {
  const id = () => props.step.id;

  return (
    <div class="stepcard-head">
      <input
        type="checkbox"
        checked={props.step.enabled}
        onChange={() => s.toggleStep(id())}
        title="Enable / disable step"
      />
      <span class="stepcard-title">{STEP_LABELS[props.step.type]}</span>
      <div class="stepcard-actions">
        <Button
          variant="icon"
          disabled={props.index === 0}
          onClick={() => s.moveStep(props.index, props.index - 1)}
          title="Move up"
        >
          ↑
        </Button>
        <Button
          variant="icon"
          disabled={props.index === props.total - 1}
          onClick={() => s.moveStep(props.index, props.index + 1)}
          title="Move down"
        >
          ↓
        </Button>
        <Button variant="icon" danger onClick={() => s.removeStep(id())} title="Remove">
          ✕
        </Button>
      </div>
    </div>
  );
}
