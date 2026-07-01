import { Match, Show, Switch } from "solid-js";

import * as s from "../store";
import { STEP_LABELS } from "../lib/steps";
import type { Scope, StepConfig } from "../lib/types";
import { Button, Checkbox, SelectField, TextField, TextareaField } from "./common";

/** Narrow `props.step` to a specific variant for typed field access. */
type Variant<T extends StepConfig["type"]> = Extract<StepConfig, { type: T }>;

export function StepCard(props: { step: StepConfig; index: number; total: number }) {
  const id = () => props.step.id;
  const set = (patch: Record<string, unknown>) => s.updateStep(id(), patch);
  const error = () => s.stepErrorFor(id());

  return (
    <div class="stepcard" classList={{ disabled: !props.step.enabled }}>
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

      <div class="stepcard-body">
        <Switch>
          <Match when={props.step.type === "findReplace"}>
            {(() => {
              const st = props.step as Variant<"findReplace">;
              return (
                <>
                  <TextField label="Find" value={st.find} onInput={(v) => set({ find: v })} />
                  <TextField label="Replace" value={st.replace} onInput={(v) => set({ replace: v })} />
                  <Checkbox
                    checked={st.caseSensitive}
                    onChange={(v) => set({ caseSensitive: v })}
                  >
                    Case sensitive
                  </Checkbox>
                  <Checkbox
                    checked={st.allOccurrences}
                    onChange={(v) => set({ allOccurrences: v })}
                  >
                    All occurrences
                  </Checkbox>
                </>
              );
            })()}
          </Match>

          <Match when={props.step.type === "regex"}>
            {(() => {
              const st = props.step as Variant<"regex">;
              return (
                <>
                  <TextField
                    label="Pattern"
                    mono
                    value={st.pattern}
                    onInput={(v) => set({ pattern: v })}
                    placeholder="(\\d+)"
                  />
                  <TextField
                    label="Replacement"
                    mono
                    value={st.replacement}
                    onInput={(v) => set({ replacement: v })}
                    placeholder="${1}"
                  />
                  <Checkbox checked={st.ignoreCase} onChange={(v) => set({ ignoreCase: v })}>
                    Ignore case
                  </Checkbox>
                  <Checkbox checked={st.multiline} onChange={(v) => set({ multiline: v })}>
                    Multiline
                  </Checkbox>
                </>
              );
            })()}
          </Match>

          <Match when={props.step.type === "changeCase"}>
            {(() => {
              const st = props.step as Variant<"changeCase">;
              return (
                <SelectField label="Mode" value={st.mode} onChange={(v) => set({ mode: v })}>
                  <option value="lower">lower case</option>
                  <option value="upper">UPPER CASE</option>
                  <option value="title">Title Case</option>
                  <option value="sentence">Sentence case</option>
                  <option value="camel">camelCase</option>
                  <option value="snake">snake_case</option>
                  <option value="kebab">kebab-case</option>
                </SelectField>
              );
            })()}
          </Match>

          <Match when={props.step.type === "insert"}>
            {(() => {
              const st = props.step as Variant<"insert">;
              return (
                <>
                  <TextField label="Text" value={st.text} onInput={(v) => set({ text: v })} />
                  <SelectField
                    label="Position"
                    value={st.position}
                    onChange={(v) => set({ position: v })}
                  >
                    <option value="prefix">Prefix</option>
                    <option value="suffix">Suffix</option>
                    <option value="atIndex">At index</option>
                  </SelectField>
                  <Show when={st.position === "atIndex"}>
                    <TextField
                      label="Index"
                      type="number"
                      value={st.index}
                      onInput={(v) => set({ index: Number(v) })}
                    />
                  </Show>
                </>
              );
            })()}
          </Match>

          <Match when={props.step.type === "remove"}>
            {(() => {
              const st = props.step as Variant<"remove">;
              return (
                <>
                  <SelectField label="From" value={st.from} onChange={(v) => set({ from: v })}>
                    <option value="start">Start</option>
                    <option value="end">End</option>
                    <option value="index">Index</option>
                  </SelectField>
                  <TextField
                    label="Count"
                    type="number"
                    min="0"
                    value={st.count}
                    onInput={(v) => set({ count: Number(v) })}
                  />
                  <Show when={st.from === "index"}>
                    <TextField
                      label="Index"
                      type="number"
                      min="0"
                      value={st.index}
                      onInput={(v) => set({ index: Number(v) })}
                    />
                  </Show>
                </>
              );
            })()}
          </Match>

          <Match when={props.step.type === "cleanUp"}>
            {(() => {
              const st = props.step as Variant<"cleanUp">;
              const spaceValue = () =>
                st.spacesTo === null ? "none" : st.spacesTo === "" ? "remove" : st.spacesTo;
              const onSpace = (v: string) =>
                set({ spacesTo: v === "none" ? null : v === "remove" ? "" : v });
              return (
                <>
                  <Checkbox checked={st.trim} onChange={(v) => set({ trim: v })}>
                    Trim ends
                  </Checkbox>
                  <Checkbox
                    checked={st.collapseWhitespace}
                    onChange={(v) => set({ collapseWhitespace: v })}
                  >
                    Collapse whitespace
                  </Checkbox>
                  <Checkbox
                    checked={st.stripDiacritics}
                    onChange={(v) => set({ stripDiacritics: v })}
                  >
                    Strip diacritics
                  </Checkbox>
                  <SelectField label="Spaces →" value={spaceValue()} onChange={onSpace}>
                    <option value="none">Keep</option>
                    <option value="-">Dash (-)</option>
                    <option value="_">Underscore (_)</option>
                    <option value="remove">Remove</option>
                  </SelectField>
                </>
              );
            })()}
          </Match>

          <Match when={props.step.type === "counter"}>
            {(() => {
              const st = props.step as Variant<"counter">;
              return (
                <>
                  <TextField
                    label="Start"
                    type="number"
                    value={st.start}
                    onInput={(v) => set({ start: Number(v) })}
                  />
                  <TextField
                    label="Step"
                    type="number"
                    value={st.step}
                    onInput={(v) => set({ step: Number(v) })}
                  />
                  <TextField
                    label="Padding"
                    type="number"
                    min="0"
                    value={st.padding}
                    onInput={(v) => set({ padding: Number(v) })}
                  />
                  <TextField
                    label="Separator"
                    value={st.separator}
                    onInput={(v) => set({ separator: v })}
                  />
                  <SelectField
                    label="Position"
                    value={st.position}
                    onChange={(v) => set({ position: v })}
                  >
                    <option value="prefix">Prefix</option>
                    <option value="suffix">Suffix</option>
                  </SelectField>
                  <Checkbox
                    checked={st.resetPerDirectory}
                    onChange={(v) => set({ resetPerDirectory: v })}
                  >
                    Reset per folder
                  </Checkbox>
                </>
              );
            })()}
          </Match>

          <Match when={props.step.type === "ai"}>
            {(() => {
              const st = props.step as Variant<"ai">;
              return (
                <>
                  <TextareaField
                    label="Prompt"
                    value={st.prompt}
                    onInput={(v) => set({ prompt: v })}
                    placeholder="e.g. Make names Title Case and human-readable"
                  />
                  <div class="ai-row">
                    <Button
                      onClick={() => s.generateAi(id(), st.prompt)}
                      disabled={
                        s.isAiLoading(id()) ||
                        s.files().length === 0 ||
                        !st.prompt.trim() ||
                        !s.activeProfile()
                      }
                    >
                      {s.isAiLoading(id()) ? "Generating…" : "Generate"}
                    </Button>
                    <span class="muted">
                      {st.results ? `${st.results.length} suggestion(s) cached` : "not run yet"}
                    </span>
                  </div>
                  <Show when={!s.activeProfile()}>
                    <p class="muted small">Set up a provider in Settings to use AI Rename.</p>
                  </Show>
                </>
              );
            })()}
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
    </div>
  );
}
