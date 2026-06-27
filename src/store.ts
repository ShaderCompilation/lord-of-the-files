// Central reactive state + actions for the renamer. Kept in one module so the reactive
// graph (files -> pipeline -> preview) lives in one place and avoids circular imports.

import { createSignal } from "solid-js";
import { createStore, produce } from "solid-js/store";

import * as ipc from "./lib/ipc";
import { defaultStep } from "./lib/steps";
import type {
  AiResultItem,
  FileEntry,
  Operation,
  PreviewResult,
  StepConfig,
  StepType,
} from "./lib/types";

// ---- Files & selection -----------------------------------------------------------------

const [files, setFiles] = createSignal<FileEntry[]>([]);
const [excluded, setExcluded] = createSignal<Set<string>>(new Set());

export { files };
export function isExcluded(id: string): boolean {
  return excluded().has(id);
}
export function toggleExclude(id: string): void {
  setExcluded((prev) => {
    const next = new Set(prev);
    next.has(id) ? next.delete(id) : next.add(id);
    return next;
  });
}

export async function addPaths(paths: string[]): Promise<void> {
  const scanned = await ipc.scanPaths(paths, recursive(), includeDirs());
  setFiles((prev) => {
    const byId = new Map(prev.map((f) => [f.id, f]));
    for (const f of scanned) byId.set(f.id, f);
    return [...byId.values()];
  });
}

export function removeFile(id: string): void {
  setFiles((prev) => prev.filter((f) => f.id !== id));
}

export function clearFiles(): void {
  setFiles([]);
  setExcluded(new Set<string>());
}

// ---- Scan / assembly options -----------------------------------------------------------

const [recursive, setRecursive] = createSignal(true);
const [includeDirs, setIncludeDirs] = createSignal(false);
const [preserveExt, setPreserveExt] = createSignal(true);
export { recursive, setRecursive, includeDirs, setIncludeDirs, preserveExt, setPreserveExt };

// ---- Pipeline --------------------------------------------------------------------------

const [pipeline, setPipeline] = createStore<{ steps: StepConfig[] }>({ steps: [] });
const [pipelineVersion, setPipelineVersion] = createSignal(0);
const bump = () => setPipelineVersion((v) => v + 1);

export { pipeline, pipelineVersion };

export function addStep(type: StepType): void {
  setPipeline("steps", (s) => [...s, defaultStep(type, preserveExt() ? "stem" : "full")]);
  bump();
}
export function removeStep(id: string): void {
  setPipeline("steps", (s) => s.filter((step) => step.id !== id));
  bump();
}
export function toggleStep(id: string): void {
  setPipeline(
    "steps",
    (step) => step.id === id,
    "enabled",
    (e) => !e,
  );
  bump();
}
export function updateStep(id: string, patch: Record<string, unknown>): void {
  setPipeline(
    "steps",
    (step) => step.id === id,
    produce((step: StepConfig) => Object.assign(step, patch)),
  );
  bump();
}
export function moveStep(from: number, to: number): void {
  setPipeline(
    "steps",
    produce((steps: StepConfig[]) => {
      if (to < 0 || to >= steps.length) return;
      const [s] = steps.splice(from, 1);
      steps.splice(to, 0, s);
    }),
  );
  bump();
}
export function clearPipeline(): void {
  setPipeline("steps", []);
  bump();
}
export function loadPipeline(steps: StepConfig[]): void {
  setPipeline("steps", steps);
  bump();
}
export function setStepResults(id: string, results: AiResultItem[] | null): void {
  updateStep(id, { results });
}

// ---- Preview ---------------------------------------------------------------------------

const [preview, setPreview] = createSignal<PreviewResult>({ rows: [], stepErrors: [] });
const [previewLoading, setPreviewLoading] = createSignal(false);
export { preview, previewLoading };

export function stepErrorFor(stepId: string): string | undefined {
  return preview().stepErrors.find((e) => e.stepId === stepId)?.message;
}

/** Recompute the preview for the current files + pipeline. */
export async function runPreview(): Promise<void> {
  const entries = files();
  if (entries.length === 0) {
    setPreview({ rows: [], stepErrors: [] });
    return;
  }
  setPreviewLoading(true);
  try {
    const result = await ipc.computePreview(entries, { steps: pipeline.steps });
    setPreview(result);
  } catch (e) {
    setNotice(`Preview failed: ${String(e)}`);
  } finally {
    setPreviewLoading(false);
  }
}

// ---- Apply / history -------------------------------------------------------------------

const [history, setHistory] = createSignal<Operation[]>([]);
const [notice, setNotice] = createSignal<string | null>(null);
export { history, notice, setNotice };

export async function refreshHistory(): Promise<void> {
  try {
    setHistory(await ipc.listOperations());
  } catch (e) {
    setNotice(`Could not load history: ${String(e)}`);
  }
}

/** Replace the filename portion of a path, preserving its separator. */
function replaceFilename(path: string, newName: string): string {
  const idx = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return idx >= 0 ? path.slice(0, idx + 1) + newName : newName;
}

function splitName(name: string): { stem: string; ext: string } {
  const dot = name.lastIndexOf(".");
  if (dot > 0) return { stem: name.slice(0, dot), ext: name.slice(dot + 1) };
  return { stem: name, ext: "" };
}

/** The rows that would actually be renamed (changed, included, no conflicts). */
export function applicableRows() {
  return preview().rows.filter((r) => r.status === "changed" && !excluded().has(r.id));
}

export async function applyAll(): Promise<void> {
  const rows = applicableRows();
  if (rows.length === 0) return;
  const items = rows.map((r) => ({ oldPath: r.id, newName: r.newName }));
  const failedPaths = new Set<string>();
  try {
    const report = await ipc.applyRename(items);
    for (const f of report.failures) failedPaths.add(f.path);

    // Update in-memory entries for the files that were renamed.
    setFiles((prev) =>
      prev.map((f) => {
        const item = items.find((i) => i.oldPath === f.id && !failedPaths.has(i.oldPath));
        if (!item) return f;
        const newPath = replaceFilename(f.path, item.newName);
        const { stem, ext } = splitName(item.newName);
        return { ...f, id: newPath, path: newPath, stem, ext };
      }),
    );
    clearPipeline();
    await refreshHistory();
    const failMsg = report.failures.length ? `, ${report.failures.length} failed` : "";
    setNotice(`Renamed ${report.renamed} file(s)${failMsg}.`);
  } catch (e) {
    setNotice(`Apply failed: ${String(e)}`);
  }
}

export async function undo(opId: string): Promise<void> {
  try {
    const r = await ipc.undoOperation(opId);
    await refreshHistory();
    setNotice(`Undid ${r.reverted} rename(s). Re-add files to continue editing.`);
  } catch (e) {
    setNotice(`Undo failed: ${String(e)}`);
  }
}

export async function redo(opId: string): Promise<void> {
  try {
    const r = await ipc.redoOperation(opId);
    await refreshHistory();
    setNotice(`Redid ${r.reverted} rename(s).`);
  } catch (e) {
    setNotice(`Redo failed: ${String(e)}`);
  }
}

// ---- AI step ---------------------------------------------------------------------------

const [aiLoading, setAiLoading] = createSignal<Set<string>>(new Set());
export function isAiLoading(stepId: string): boolean {
  return aiLoading().has(stepId);
}
function setAiBusy(stepId: string, busy: boolean) {
  setAiLoading((prev) => {
    const next = new Set(prev);
    busy ? next.add(stepId) : next.delete(stepId);
    return next;
  });
}

export async function generateAi(stepId: string, prompt: string): Promise<void> {
  const entries = files();
  if (entries.length === 0 || !prompt.trim()) return;
  setAiBusy(stepId, true);
  try {
    const results = await ipc.aiGenerate(prompt, entries, 80);
    setStepResults(stepId, results);
    setNotice(`AI suggested ${results.length} name(s).`);
  } catch (e) {
    setNotice(`AI request failed: ${String(e)}`);
  } finally {
    setAiBusy(stepId, false);
  }
}
