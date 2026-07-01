// Step metadata and factory for default configs.

import type { CaseMode, Scope, StepConfig, StepType } from "./types";

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

/** Compact glyph shown in the add-step menu and on each step card. */
export const STEP_ICONS: Record<StepType, string> = {
  findReplace: "⇄",
  regex: ".*",
  changeCase: "Aa",
  insert: "T+",
  remove: "⌫",
  cleanUp: "✧",
  counter: "#",
  ai: "✦",
};

/** One-line explanation shown next to each item in the add-step menu. */
export const STEP_DESCRIPTIONS: Record<StepType, string> = {
  findReplace: "Swap literal text for something else",
  regex: "Match and replace with a regular expression",
  changeCase: "Convert to Title, snake_case, kebab-case…",
  insert: "Add a prefix, suffix, or text at a position",
  remove: "Delete characters from the start, end, or index",
  cleanUp: "Trim, collapse spaces, strip accents",
  counter: "Append or prepend a running number",
  ai: "Describe what you want and let a model suggest names",
};

/** Add-step menu, grouped by intent. AI is set apart so it reads as its own path. */
export const STEP_GROUPS: { label: string; ai?: boolean; types: StepType[] }[] = [
  { label: "Text", types: ["findReplace", "regex", "insert", "remove"] },
  { label: "Format", types: ["changeCase", "cleanUp"] },
  { label: "Number", types: ["counter"] },
  { label: "AI", ai: true, types: ["ai"] },
];

const CASE_LABELS: Record<CaseMode, string> = {
  lower: "lowercase",
  upper: "UPPERCASE",
  title: "Title Case",
  sentence: "Sentence case",
  camel: "camelCase",
  snake: "snake_case",
  kebab: "kebab-case",
};

/** Short label for the scope a step operates on. */
export function scopeLabel(scope: Scope): string {
  return scope === "stem" ? "Name" : scope === "ext" ? "Ext" : "Full";
}

/** Human-readable one-liner for a collapsed step card. */
export function stepSummary(step: StepConfig): string {
  const scope = scopeLabel(step.scope);
  let body: string;
  switch (step.type) {
    case "findReplace":
      body = `Find “${step.find || "…"}” → “${step.replace}”`;
      break;
    case "regex":
      body = `/${step.pattern || "…"}/ → ${step.replacement || "…"}`;
      break;
    case "changeCase":
      body = CASE_LABELS[step.mode];
      break;
    case "insert": {
      const where =
        step.position === "atIndex" ? `at ${step.index}` : step.position;
      body = `Insert “${step.text || "…"}” (${where})`;
      break;
    }
    case "remove": {
      const where = step.from === "index" ? `index ${step.index}` : step.from;
      body = `Remove ${step.count} from ${where}`;
      break;
    }
    case "cleanUp": {
      const ops: string[] = [];
      if (step.trim) ops.push("Trim");
      if (step.collapseWhitespace) ops.push("Collapse spaces");
      if (step.spacesTo !== null) ops.push(`Spaces → “${step.spacesTo}”`);
      if (step.stripDiacritics) ops.push("Strip accents");
      body = ops.length ? ops.join(" · ") : "No-op";
      break;
    }
    case "counter":
      body = `#${step.start}, step ${step.step} (${step.position})`;
      break;
    case "ai":
      body = step.prompt
        ? `“${step.prompt.length > 40 ? step.prompt.slice(0, 40) + "…" : step.prompt}”`
        : "No prompt yet";
      break;
  }
  return `${body} · ${scope}`;
}

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
