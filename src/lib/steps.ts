// Step metadata and factory for default configs.

import type { Scope, StepConfig, StepType } from "./types";

let seq = 0;
export function newId(): string {
  return `step-${Date.now().toString(36)}-${seq++}`;
}

export const STEP_LABELS: Record<StepType, string> = {
  findReplace: "Find & Replace",
  regex: "Regex Replace",
  changeCase: "Change Case",
  insert: "Insert Text",
  remove: "Remove Text",
  cleanUp: "Clean Up",
  counter: "Counter",
  ai: "AI Rename",
};

export const STEP_ORDER: StepType[] = [
  "findReplace",
  "regex",
  "changeCase",
  "insert",
  "remove",
  "cleanUp",
  "counter",
  "ai",
];

/** Build a fresh step of the given type. `scope` defaults from the preserve-extension UI. */
export function defaultStep(type: StepType, scope: Scope = "stem"): StepConfig {
  const base = { id: newId(), enabled: true, scope };
  switch (type) {
    case "findReplace":
      return { ...base, type, find: "", replace: "", caseSensitive: true, allOccurrences: true };
    case "regex":
      return {
        ...base,
        type,
        pattern: "",
        replacement: "",
        ignoreCase: false,
        dotall: false,
        multiline: false,
      };
    case "changeCase":
      return { ...base, type, mode: "title" };
    case "insert":
      return { ...base, type, text: "", position: "prefix", index: 0 };
    case "remove":
      return { ...base, type, from: "start", count: 1, index: 0 };
    case "cleanUp":
      return {
        ...base,
        type,
        trim: true,
        collapseWhitespace: true,
        spacesTo: null,
        stripDiacritics: false,
      };
    case "counter":
      return {
        ...base,
        type,
        start: 1,
        step: 1,
        padding: 3,
        separator: "_",
        position: "suffix",
        resetPerDirectory: false,
      };
    case "ai":
      return { ...base, type, prompt: "", results: null };
  }
}
