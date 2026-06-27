import { For, Show, createSignal } from "solid-js";

import * as s from "../store";
import { STEP_LABELS, STEP_ORDER } from "../lib/steps";
import { deletePreset, instantiate, loadPresets, savePreset } from "../lib/presets";
import { StepCard } from "./StepCard";

export function PipelineEditor() {
  const [presets, setPresets] = createSignal<Record<string, ReturnType<typeof loadPresets>[string]>>(
    loadPresets(),
  );
  const [selected, setSelected] = createSignal("");

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

  return (
    <aside class="pipeline">
      <div class="pipeline-head">
        <h2>Pipeline</h2>
        <Show when={s.pipeline.steps.length > 0}>
          <button type="button" class="ghost small" onClick={s.clearPipeline}>
            Clear
          </button>
        </Show>
      </div>

      <div class="presets">
        <select value={selected()} onChange={(e) => onLoad(e.currentTarget.value)}>
          <option value="">Presets…</option>
          <For each={Object.keys(presets())}>{(name) => <option value={name}>{name}</option>}</For>
        </select>
        <button type="button" class="ghost small" onClick={onSave} disabled={s.pipeline.steps.length === 0}>
          Save
        </button>
        <button type="button" class="ghost small" onClick={onDelete} disabled={!selected()}>
          Delete
        </button>
      </div>

      <div class="add-steps">
        <For each={STEP_ORDER}>
          {(type) => (
            <button type="button" class="chip" onClick={() => s.addStep(type)}>
              + {STEP_LABELS[type]}
            </button>
          )}
        </For>
      </div>

      <div class="steps">
        <Show
          when={s.pipeline.steps.length > 0}
          fallback={<p class="muted hint">Add a step to start transforming names.</p>}
        >
          <For each={s.pipeline.steps}>
            {(step, i) => (
              <StepCard step={step} index={i()} total={s.pipeline.steps.length} />
            )}
          </For>
        </Show>
      </div>
    </aside>
  );
}
