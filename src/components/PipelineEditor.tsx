import { For, Show, createSignal } from "solid-js";

import * as s from "../store";
import { deletePreset, instantiate, loadPresets, savePreset } from "../lib/presets";
import { Button, Checkbox } from "./common";
import { AddStepMenu } from "./AddStepMenu";
import { StepCard } from "./StepCard";

export function PipelineEditor() {
  const [presets, setPresets] = createSignal<Record<string, ReturnType<typeof loadPresets>[string]>>(
    loadPresets(),
  );
  const [selected, setSelected] = createSignal("");

  // Drag-to-reorder state: which card is being dragged and which slot it's hovering.
  const [dragIndex, setDragIndex] = createSignal<number | null>(null);
  const [overIndex, setOverIndex] = createSignal<number | null>(null);

  const refreshPresets = () => setPresets(loadPresets());

  const onLoad = (name: string) => {
    setSelected(name);
    const steps = presets()[name];
    if (steps) s.loadPipeline(instantiate(steps));
  };

  const onSave = () => {
    const name = window.prompt("Preset name:");
    if (!name) return;
    savePreset(name, s.pipeline.steps);
    refreshPresets();
    setSelected(name);
  };

  const onDelete = () => {
    const name = selected();
    if (!name) return;
    deletePreset(name);
    refreshPresets();
    setSelected("");
  };

  const dropOn = (target: number) => {
    const from = dragIndex();
    if (from !== null && from !== target) s.moveStep(from, target);
    setDragIndex(null);
    setOverIndex(null);
  };

  const hasFiles = () => s.files().length > 0;
  const hasSteps = () => s.pipeline.steps.length > 0;

  return (
    <aside class="pipeline">
      <div class="pipeline-head">
        <h2>Recipe</h2>
        <Show when={hasFiles()}>
          <div class="pipeline-head-actions">
            <Checkbox checked={s.preserveExt()} onChange={s.setPreserveExt} title="Keep the file extension untouched">
              Preserve extension
            </Checkbox>
            <Show when={hasSteps()}>
              <Button variant="ghost" small onClick={s.clearPipeline}>
                Clear
              </Button>
            </Show>
          </div>
        </Show>
      </div>

      <Show
        when={hasFiles()}
        fallback={
          <div class="pipeline-locked">
            <p>Add files first</p>
          </div>
        }
      >
        <div class="presets">
          <select value={selected()} onChange={(e) => onLoad(e.currentTarget.value)}>
            <option value="">Presets…</option>
            <For each={Object.keys(presets())}>{(name) => <option value={name}>{name}</option>}</For>
          </select>
          <Button variant="ghost" small onClick={onSave} disabled={!hasSteps()}>
            Save
          </Button>
          <Button variant="ghost" small onClick={onDelete} disabled={!selected()}>
            Delete
          </Button>
        </div>

        <Show
          when={hasSteps()}
          fallback={
            <div class="method-chooser">
              <div class="method-chooser-title">Choose how to rename</div>
              <button type="button" class="method-card" onClick={() => s.addStep("findReplace")}>
                <span class="method-icon">🧩</span>
                <span class="method-text">
                  <span class="method-name">Build a recipe</span>
                  <span class="method-desc">Stack find &amp; replace, case, counters and more.</span>
                </span>
              </button>
              <button type="button" class="method-card ai" onClick={() => s.addStep("ai")}>
                <span class="method-icon">✦</span>
                <span class="method-text">
                  <span class="method-name">AI Rename</span>
                  <span class="method-desc">Describe the names you want in plain language.</span>
                </span>
              </button>
            </div>
          }
        >
          <div class="steps-toolbar">
            <AddStepMenu />
          </div>
          <div class="steps">
            <For each={s.pipeline.steps}>
              {(step, i) => (
                <div
                  class="step-wrap"
                  classList={{ "drag-over": overIndex() === i() && dragIndex() !== i() }}
                  onDragOver={(e) => {
                    if (dragIndex() === null) return;
                    e.preventDefault();
                    setOverIndex(i());
                  }}
                  onDrop={(e) => {
                    e.preventDefault();
                    dropOn(i());
                  }}
                >
                  <StepCard
                    step={step}
                    index={i()}
                    onDragStart={() => setDragIndex(i())}
                    onDragEnd={() => {
                      setDragIndex(null);
                      setOverIndex(null);
                    }}
                  />
                </div>
              )}
            </For>
          </div>
        </Show>
      </Show>
    </aside>
  );
}
