import { Match, Show, Switch, createMemo, createSignal } from "solid-js";

import * as s from "../../store";
import { stepSummary } from "../../lib/steps";
import type { Scope, StepConfig } from "../../lib/types";
import { SelectField } from "../common";
import { AiFields } from "./AiFields";
import { ChangeCaseFields } from "./ChangeCaseFields";
import { CleanUpFields } from "./CleanUpFields";
import { CounterFields } from "./CounterFields";
import { FindReplaceFields } from "./FindReplaceFields";
import { InsertFields } from "./InsertFields";
import { RegexFields } from "./RegexFields";
import { RemoveFields } from "./RemoveFields";
import { StepCardHead } from "./StepCardHead";
import type { Variant } from "./types";

export function StepCard(props: {
  step: StepConfig;
  index: number;
  onDragStart?: () => void;
  onDragEnd?: () => void;
}) {
  const id = () => props.step.id;
  const set = (patch: Record<string, unknown>) => s.updateStep(id(), patch);
  const error = () => s.stepErrorFor(id());
  const [collapsed, setCollapsed] = createSignal(false);
  const summary = createMemo(() => stepSummary(props.step));

  return (
    <div
      class="stepcard"
      classList={{
        disabled: !props.step.enabled,
        ai: props.step.type === "ai",
        collapsed: collapsed(),
      }}
    >
      <StepCardHead
        step={props.step}
        index={props.index}
        collapsed={collapsed()}
        summary={summary()}
        onToggleCollapse={() => setCollapsed((v) => !v)}
        onDragStart={props.onDragStart}
        onDragEnd={props.onDragEnd}
      />

      <Show when={!collapsed()}>
      <div class="stepcard-body">
        <Switch>
          <Match when={props.step.type === "findReplace"}>
            <FindReplaceFields step={props.step as Variant<"findReplace">} set={set} />
          </Match>

          <Match when={props.step.type === "regex"}>
            <RegexFields step={props.step as Variant<"regex">} set={set} />
          </Match>

          <Match when={props.step.type === "changeCase"}>
            <ChangeCaseFields step={props.step as Variant<"changeCase">} set={set} />
          </Match>

          <Match when={props.step.type === "insert"}>
            <InsertFields step={props.step as Variant<"insert">} set={set} />
          </Match>

          <Match when={props.step.type === "remove"}>
            <RemoveFields step={props.step as Variant<"remove">} set={set} />
          </Match>

          <Match when={props.step.type === "cleanUp"}>
            <CleanUpFields step={props.step as Variant<"cleanUp">} set={set} />
          </Match>

          <Match when={props.step.type === "counter"}>
            <CounterFields step={props.step as Variant<"counter">} set={set} />
          </Match>

          <Match when={props.step.type === "ai"}>
            <AiFields step={props.step as Variant<"ai">} set={set} id={id()} />
          </Match>
        </Switch>

        <div class="stepcard-footer">
          <SelectField
            label="Scope"
            class="scope"
            value={props.step.scope}
            onChange={(v) => set({ scope: v as Scope })}
          >
            <option value="stem">Name</option>
            <option value="ext">Extension</option>
            <option value="full">Full name</option>
          </SelectField>
        </div>

        <Show when={error()}>
          <div class="step-error">{error()}</div>
        </Show>
      </div>
      </Show>
    </div>
  );
}
