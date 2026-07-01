import { Match, Show, Switch } from "solid-js";

import * as s from "../store";
import { STEP_LABELS } from "../lib/steps";
import type { Scope, StepConfig } from "../lib/types";

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
          <button type="button"
            class="icon"
            disabled={props.index === 0}
            onClick={() => s.moveStep(props.index, props.index - 1)}
            title="Move up"
          >
            ↑
          </button>
          <button type="button"
            class="icon"
            disabled={props.index === props.total - 1}
            onClick={() => s.moveStep(props.index, props.index + 1)}
            title="Move down"
          >
            ↓
          </button>
          <button type="button" class="icon danger" onClick={() => s.removeStep(id())} title="Remove">
            ✕
          </button>
        </div>
      </div>

      <div class="stepcard-body">
        <Switch>
          <Match when={props.step.type === "findReplace"}>
            {(() => {
              const st = props.step as Variant<"findReplace">;
              return (
                <>
                  <label class="field">
                    Find
                    <input value={st.find} onInput={(e) => set({ find: e.currentTarget.value })} />
                  </label>
                  <label class="field">
                    Replace
                    <input
                      value={st.replace}
                      onInput={(e) => set({ replace: e.currentTarget.value })}
                    />
                  </label>
                  <label class="check">
                    <input
                      type="checkbox"
                      checked={st.caseSensitive}
                      onChange={(e) => set({ caseSensitive: e.currentTarget.checked })}
                    />
                    Case sensitive
                  </label>
                  <label class="check">
                    <input
                      type="checkbox"
                      checked={st.allOccurrences}
                      onChange={(e) => set({ allOccurrences: e.currentTarget.checked })}
                    />
                    All occurrences
                  </label>
                </>
              );
            })()}
          </Match>

          <Match when={props.step.type === "regex"}>
            {(() => {
              const st = props.step as Variant<"regex">;
              return (
                <>
                  <label class="field">
                    Pattern
                    <input
                      class="mono"
                      value={st.pattern}
                      onInput={(e) => set({ pattern: e.currentTarget.value })}
                      placeholder="(\\d+)"
                    />
                  </label>
                  <label class="field">
                    Replacement
                    <input
                      class="mono"
                      value={st.replacement}
                      onInput={(e) => set({ replacement: e.currentTarget.value })}
                      placeholder="${1}"
                    />
                  </label>
                  <label class="check">
                    <input
                      type="checkbox"
                      checked={st.ignoreCase}
                      onChange={(e) => set({ ignoreCase: e.currentTarget.checked })}
                    />
                    Ignore case
                  </label>
                  <label class="check">
                    <input
                      type="checkbox"
                      checked={st.multiline}
                      onChange={(e) => set({ multiline: e.currentTarget.checked })}
                    />
                    Multiline
                  </label>
                </>
              );
            })()}
          </Match>

          <Match when={props.step.type === "changeCase"}>
            {(() => {
              const st = props.step as Variant<"changeCase">;
              return (
                <label class="field">
                  Mode
                  <select value={st.mode} onChange={(e) => set({ mode: e.currentTarget.value })}>
                    <option value="lower">lower case</option>
                    <option value="upper">UPPER CASE</option>
                    <option value="title">Title Case</option>
                    <option value="sentence">Sentence case</option>
                    <option value="camel">camelCase</option>
                    <option value="snake">snake_case</option>
                    <option value="kebab">kebab-case</option>
                  </select>
                </label>
              );
            })()}
          </Match>

          <Match when={props.step.type === "insert"}>
            {(() => {
              const st = props.step as Variant<"insert">;
              return (
                <>
                  <label class="field">
                    Text
                    <input value={st.text} onInput={(e) => set({ text: e.currentTarget.value })} />
                  </label>
                  <label class="field">
                    Position
                    <select
                      value={st.position}
                      onChange={(e) => set({ position: e.currentTarget.value })}
                    >
                      <option value="prefix">Prefix</option>
                      <option value="suffix">Suffix</option>
                      <option value="atIndex">At index</option>
                    </select>
                  </label>
                  <Show when={st.position === "atIndex"}>
                    <label class="field">
                      Index
                      <input
                        type="number"
                        value={st.index}
                        onInput={(e) => set({ index: Number(e.currentTarget.value) })}
                      />
                    </label>
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
                  <label class="field">
                    From
                    <select value={st.from} onChange={(e) => set({ from: e.currentTarget.value })}>
                      <option value="start">Start</option>
                      <option value="end">End</option>
                      <option value="index">Index</option>
                    </select>
                  </label>
                  <label class="field">
                    Count
                    <input
                      type="number"
                      min="0"
                      value={st.count}
                      onInput={(e) => set({ count: Number(e.currentTarget.value) })}
                    />
                  </label>
                  <Show when={st.from === "index"}>
                    <label class="field">
                      Index
                      <input
                        type="number"
                        min="0"
                        value={st.index}
                        onInput={(e) => set({ index: Number(e.currentTarget.value) })}
                      />
                    </label>
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
                  <label class="check">
                    <input
                      type="checkbox"
                      checked={st.trim}
                      onChange={(e) => set({ trim: e.currentTarget.checked })}
                    />
                    Trim ends
                  </label>
                  <label class="check">
                    <input
                      type="checkbox"
                      checked={st.collapseWhitespace}
                      onChange={(e) => set({ collapseWhitespace: e.currentTarget.checked })}
                    />
                    Collapse whitespace
                  </label>
                  <label class="check">
                    <input
                      type="checkbox"
                      checked={st.stripDiacritics}
                      onChange={(e) => set({ stripDiacritics: e.currentTarget.checked })}
                    />
                    Strip diacritics
                  </label>
                  <label class="field">
                    Spaces →
                    <select value={spaceValue()} onChange={(e) => onSpace(e.currentTarget.value)}>
                      <option value="none">Keep</option>
                      <option value="-">Dash (-)</option>
                      <option value="_">Underscore (_)</option>
                      <option value="remove">Remove</option>
                    </select>
                  </label>
                </>
              );
            })()}
          </Match>

          <Match when={props.step.type === "counter"}>
            {(() => {
              const st = props.step as Variant<"counter">;
              return (
                <>
                  <label class="field">
                    Start
                    <input
                      type="number"
                      value={st.start}
                      onInput={(e) => set({ start: Number(e.currentTarget.value) })}
                    />
                  </label>
                  <label class="field">
                    Step
                    <input
                      type="number"
                      value={st.step}
                      onInput={(e) => set({ step: Number(e.currentTarget.value) })}
                    />
                  </label>
                  <label class="field">
                    Padding
                    <input
                      type="number"
                      min="0"
                      value={st.padding}
                      onInput={(e) => set({ padding: Number(e.currentTarget.value) })}
                    />
                  </label>
                  <label class="field">
                    Separator
                    <input
                      value={st.separator}
                      onInput={(e) => set({ separator: e.currentTarget.value })}
                    />
                  </label>
                  <label class="field">
                    Position
                    <select
                      value={st.position}
                      onChange={(e) => set({ position: e.currentTarget.value })}
                    >
                      <option value="prefix">Prefix</option>
                      <option value="suffix">Suffix</option>
                    </select>
                  </label>
                  <label class="check">
                    <input
                      type="checkbox"
                      checked={st.resetPerDirectory}
                      onChange={(e) => set({ resetPerDirectory: e.currentTarget.checked })}
                    />
                    Reset per folder
                  </label>
                </>
              );
            })()}
          </Match>

          <Match when={props.step.type === "ai"}>
            {(() => {
              const st = props.step as Variant<"ai">;
              return (
                <>
                  <label class="field">
                    Prompt
                    <textarea
                      rows="3"
                      value={st.prompt}
                      onInput={(e) => set({ prompt: e.currentTarget.value })}
                      placeholder="e.g. Make names Title Case and human-readable"
                    />
                  </label>
                  <div class="ai-row">
                    <button type="button"
                      onClick={() => s.generateAi(id(), st.prompt)}
                      disabled={
                        s.isAiLoading(id()) ||
                        s.files().length === 0 ||
                        !st.prompt.trim() ||
                        !s.activeProfile()
                      }
                    >
                      {s.isAiLoading(id()) ? "Generating…" : "Generate"}
                    </button>
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
          <label class="field scope">
            Scope
            <select
              value={props.step.scope}
              onChange={(e) => set({ scope: e.currentTarget.value as Scope })}
            >
              <option value="stem">Name</option>
              <option value="ext">Extension</option>
              <option value="full">Full name</option>
            </select>
          </label>
        </div>

        <Show when={error()}>
          <div class="step-error">{error()}</div>
        </Show>
      </div>
    </div>
  );
}
