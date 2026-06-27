// Named pipeline presets, persisted in localStorage as JSON.

import { newId } from "./steps";
import type { StepConfig } from "./types";

const KEY = "lotf.presets";

export type PresetMap = Record<string, StepConfig[]>;

export function loadPresets(): PresetMap {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}") as PresetMap;
  } catch {
    return {};
  }
}

export function savePreset(name: string, steps: StepConfig[]): void {
  const all = loadPresets();
  all[name] = steps;
  localStorage.setItem(KEY, JSON.stringify(all));
}

export function deletePreset(name: string): void {
  const all = loadPresets();
  delete all[name];
  localStorage.setItem(KEY, JSON.stringify(all));
}

/** Clone preset steps with fresh ids so they don't collide with the live pipeline. */
export function instantiate(steps: StepConfig[]): StepConfig[] {
  return steps.map((s) => ({ ...s, id: newId() }));
}
