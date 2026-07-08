// Central reactive state + actions for the renamer. Kept in one module so the reactive
// graph (files -> pipeline -> preview) lives in one place and avoids circular imports.

import { open } from "@tauri-apps/plugin-dialog";
import { createSignal } from "solid-js";
import { createStore, produce } from "solid-js/store";

import * as ipc from "./lib/ipc";
import { log } from "./lib/log";
import { defaultStep } from "./lib/steps";
import type {
  AiGenerationDetail,
  AiGenerationSummary,
  AiResultItem,
  FileCheck,
  Failure,
  FileEntry,
  MockAiConfig,
  Operation,
  PreviewResult,
  ProviderProfile,
  RenameEntry,
  SettingsState,
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
  log.debug(`addPaths: ${paths.length} path(s)`);
  try {
    const scanned = await ipc.scanPaths(paths, recursive(), includeDirs());
    setFiles((prev) => {
      const byId = new Map(prev.map((f) => [f.id, f]));
      for (const f of scanned) byId.set(f.id, f);
      return [...byId.values()];
    });
    markPreviewStale();
    log.debug(`addPaths: scanned ${scanned.length} entries, ${files().length} total`);
  } catch (e) {
    log.error(`addPaths failed: ${String(e)}`);
    setNotice(`Could not scan paths: ${String(e)}`);
  }
}

/** Open the native file picker and add the selection. */
export async function pickFiles(): Promise<void> {
  const sel = await open({ multiple: true });
  if (sel) await addPaths(Array.isArray(sel) ? sel : [sel]);
}

/** Open the native folder picker and add the selection. */
export async function pickFolder(): Promise<void> {
  const sel = await open({ directory: true, multiple: true });
  if (sel) await addPaths(Array.isArray(sel) ? sel : [sel]);
}

export function removeFile(id: string): void {
  setFiles((prev) => prev.filter((f) => f.id !== id));
  markPreviewStale();
}

export function clearFiles(): void {
  setFiles([]);
  setExcluded(new Set<string>());
  setTableFilter("all");
  markPreviewStale();
  setPreview({ rows: [], stepErrors: [] });
  setPreviewAppliedVersion(previewInputVersion());
}

// ---- Scan / assembly options -----------------------------------------------------------

const [recursive, setRecursive] = createSignal(true);
const [includeDirs, setIncludeDirs] = createSignal(false);
const [preserveExt, setPreserveExt] = createSignal(true);
export { recursive, setRecursive, includeDirs, setIncludeDirs, preserveExt, setPreserveExt };

// ---- Pipeline --------------------------------------------------------------------------

const [pipeline, setPipeline] = createStore<{ steps: StepConfig[] }>({ steps: [] });
const [pipelineVersion, setPipelineVersion] = createSignal(0);
const [previewInputVersion, setPreviewInputVersion] = createSignal(0);
const [previewAppliedVersion, setPreviewAppliedVersion] = createSignal(0);
let previewRequestId = 0;

function markPreviewStale(): void {
  setPreviewInputVersion((v) => v + 1);
}

const bump = () => {
  setPipelineVersion((v) => v + 1);
  markPreviewStale();
};

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

export function previewStale(): boolean {
  return files().length > 0 && previewAppliedVersion() !== previewInputVersion();
}

export function previewReady(): boolean {
  return !previewLoading() && !previewStale();
}

// Which preview rows the file table shows. Lifted into the store so the toolbar's
// "conflicts" affordance can jump the table straight to the blocking rows.
export type TableFilter = "all" | "changed" | "conflict" | "unchanged";
const [tableFilter, setTableFilter] = createSignal<TableFilter>("all");
export { tableFilter, setTableFilter };

export interface PreviewCounts {
  total: number;
  changed: number;
  conflict: number;
  unchanged: number;
}

/** Row counts by status (conflict groups both `conflict` and `invalid`). */
export function previewCounts(): PreviewCounts {
  const counts: PreviewCounts = { total: 0, changed: 0, conflict: 0, unchanged: 0 };
  for (const r of preview().rows) {
    counts.total++;
    if (r.status === "changed") counts.changed++;
    else if (r.status === "conflict" || r.status === "invalid") counts.conflict++;
    else counts.unchanged++;
  }
  return counts;
}

export function stepErrorFor(stepId: string): string | undefined {
  return preview().stepErrors.find((e) => e.stepId === stepId)?.message;
}

/** Recompute the preview for the current files + pipeline. */
export async function runPreview(): Promise<void> {
  const entries = files();
  const version = previewInputVersion();
  const requestId = ++previewRequestId;
  if (entries.length === 0) {
    setPreview({ rows: [], stepErrors: [] });
    setPreviewAppliedVersion(version);
    setPreviewLoading(false);
    return;
  }
  log.debug(`runPreview: ${entries.length} entries, ${pipeline.steps.length} step(s)`);
  setPreviewLoading(true);
  try {
    const result = await ipc.computePreview(entries, { steps: pipeline.steps });
    if (requestId !== previewRequestId || version !== previewInputVersion()) {
      log.debug(`runPreview: ignored stale result request=${requestId}`);
      return;
    }
    setPreview(result);
    setPreviewAppliedVersion(version);
    if (result.stepErrors.length > 0) {
      for (const err of result.stepErrors) {
        log.warn(`runPreview step error: stepId=${err.stepId} ${err.message}`);
      }
    }
  } catch (e) {
    if (requestId === previewRequestId) {
      log.error(`runPreview failed: ${String(e)}`);
      setNotice(`Preview failed: ${String(e)}`);
    }
  } finally {
    if (requestId === previewRequestId) setPreviewLoading(false);
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
    log.error(`refreshHistory failed: ${String(e)}`);
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
  if (!previewReady()) {
    await runPreview();
  }
  if (!previewReady()) {
    setNotice("Preview is still updating. Try again when it finishes.");
    return;
  }
  const rows = applicableRows();
  if (rows.length === 0) return;
  const items = rows.map((r) => ({ oldPath: r.id, newName: r.newName }));
  log.info(`applyAll: ${items.length} item(s)`);
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
    markPreviewStale();
    await refreshHistory();
    for (const f of report.failures) {
      log.warn(`applyAll failed: ${f.path}: ${f.error}`);
    }
    log.info(`applyAll done: renamed=${report.renamed}, failed=${report.failures.length}`);
    const failMsg = report.failures.length ? `, ${report.failures.length} failed` : "";
    setNotice(`Renamed ${report.renamed} file(s)${failMsg}.`);
  } catch (e) {
    log.error(`applyAll failed: ${String(e)}`);
    setNotice(`Apply failed: ${String(e)}`);
  }
}

// ---- History: detail modal (per-op file list, opened by clicking a history row) -------

const [historyDetailOpId, setHistoryDetailOpId] = createSignal<string | null>(null);
const [opFiles, setOpFiles] = createSignal<Map<string, RenameEntry[]>>(new Map());
const [opFilesLoading, setOpFilesLoading] = createSignal<Set<string>>(new Set());
export { historyDetailOpId, opFiles };

export function isOpFilesLoading(opId: string): boolean {
  return opFilesLoading().has(opId);
}

async function fetchOpFiles(opId: string): Promise<void> {
  setOpFilesLoading((prev) => new Set(prev).add(opId));
  try {
    const files = await ipc.getOperationFiles(opId);
    setOpFiles((prev) => new Map(prev).set(opId, files));
  } catch (e) {
    log.error(`fetchOpFiles failed: ${String(e)}`);
    setNotice(`Could not load file list: ${String(e)}`);
  } finally {
    setOpFilesLoading((prev) => {
      const next = new Set(prev);
      next.delete(opId);
      return next;
    });
  }
}

export function openHistoryDetail(opId: string): void {
  setHistoryDetailOpId(opId);
  if (!opFiles().has(opId)) void fetchOpFiles(opId);
}

export function closeHistoryDetail(): void {
  setHistoryDetailOpId(null);
  setPendingAction(null);
}

// ---- History: confirm-before-undo/redo (live filesystem dry-run preview) --------------

export interface PendingAction {
  opId: string;
  direction: "undo" | "redo";
  checks: FileCheck[];
}
const [pendingAction, setPendingAction] = createSignal<PendingAction | null>(null);
const [pendingLoading, setPendingLoading] = createSignal(false); // dry-run fetch in flight
const [confirmBusy, setConfirmBusy] = createSignal(false); // actual undo/redo in flight
const [opErrors, setOpErrors] = createSignal<Map<string, Failure[]>>(new Map());
export { pendingAction, pendingLoading, confirmBusy, opErrors };

export async function requestUndo(opId: string): Promise<void> {
  if (pendingLoading() || confirmBusy()) return;
  log.debug(`requestUndo: ${opId}`);
  openHistoryDetail(opId); // also warms the file-list cache, used if the user cancels
  setPendingLoading(true);
  try {
    const checks = await ipc.previewUndo(opId);
    setPendingAction({ opId, direction: "undo", checks });
  } catch (e) {
    log.error(`previewUndo failed: ${String(e)}`);
    setNotice(`Could not preview undo: ${String(e)}`);
  } finally {
    setPendingLoading(false);
  }
}

export async function requestRedo(opId: string): Promise<void> {
  if (pendingLoading() || confirmBusy()) return;
  log.debug(`requestRedo: ${opId}`);
  openHistoryDetail(opId); // also warms the file-list cache, used if the user cancels
  setPendingLoading(true);
  try {
    const checks = await ipc.previewRedo(opId);
    setPendingAction({ opId, direction: "redo", checks });
  } catch (e) {
    log.error(`previewRedo failed: ${String(e)}`);
    setNotice(`Could not preview redo: ${String(e)}`);
  } finally {
    setPendingLoading(false);
  }
}

export function cancelPendingAction(): void {
  setPendingAction(null);
}

export async function confirmPendingAction(): Promise<void> {
  const pending = pendingAction();
  if (!pending || confirmBusy()) return;
  setConfirmBusy(true);
  try {
    const r =
      pending.direction === "undo"
        ? await ipc.undoOperation(pending.opId)
        : await ipc.redoOperation(pending.opId);

    // The dry-run already told the user which files were missing/conflicting; only
    // surface failures that weren't already predicted, so the same problem isn't
    // reported twice.
    const expected = new Set(
      pending.checks.filter((c) => c.status !== "ok").map((c) => c.oldPath),
    );
    const unexpected = r.failures.filter((f) => !expected.has(f.path));
    setOpErrors((prev) => {
      const next = new Map(prev);
      if (unexpected.length) next.set(pending.opId, unexpected);
      else next.delete(pending.opId);
      return next;
    });

    await refreshHistory();
    await fetchOpFiles(pending.opId); // refresh the cached list so the modal shows the new state
    const verb = pending.direction === "undo" ? "Undid" : "Redid";
    log.info(
      `${pending.direction}: opId=${pending.opId}, reverted=${r.reverted}, failed=${r.failures.length}`,
    );
    for (const f of r.failures) {
      log.warn(`${pending.direction} failed: ${f.path}: ${f.error}`);
    }
    const failMsg = r.failures.length ? `, ${r.failures.length} failed` : "";
    setNotice(`${verb} ${r.reverted} rename(s)${failMsg}.`);
  } catch (e) {
    log.error(`confirmPendingAction failed: ${String(e)}`);
    setNotice(`Action failed: ${String(e)}`);
  } finally {
    setConfirmBusy(false);
    setPendingAction(null);
  }
}

// ---- AI step ---------------------------------------------------------------------------

const [aiLoading, setAiLoading] = createSignal<Set<string>>(new Set());
const [aiStepError, setAiStepError] = createSignal<Map<string, string>>(new Map());
const [aiGenerationId, setAiGenerationId] = createSignal<Map<string, string>>(new Map());
// Unlike `aiGenerationId` (cleared on cancel, used for live/race checks), this survives
// cancellation/completion so the step's "Details" button can keep referencing the most recent
// generation even after it's done or was cancelled.
const [aiLastGenerationId, setAiLastGenerationId] = createSignal<Map<string, string>>(new Map());

export function isAiLoading(stepId: string): boolean {
  return aiLoading().has(stepId);
}
export function aiErrorFor(stepId: string): string | undefined {
  return aiStepError().get(stepId);
}
export function hasAiGeneration(stepId: string): boolean {
  return aiLastGenerationId().has(stepId);
}
export function lastAiGenerationId(stepId: string): string | undefined {
  return aiLastGenerationId().get(stepId);
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
  if (entries.length === 0 || !prompt.trim()) {
    log.debug(`generateAi: skipped step=${stepId} (empty files or prompt)`);
    return;
  }
  if (!activeProfile()) {
    log.debug(`generateAi: skipped step=${stepId} (no active provider)`);
    setNotice("No active provider — open Settings to add one.");
    return;
  }
  const generationId = crypto.randomUUID();
  const profile = activeProfile()!;
  const prevGenerationId = aiGenerationId().get(stepId);
  if (prevGenerationId) {
    log.info(
      `generateAi: superseding in-flight generation=${prevGenerationId} for step=${stepId}`,
    );
    void ipc.cancelAiGenerate(prevGenerationId).catch((e) => {
      log.debug(
        `generateAi: best-effort cancel of superseded generation=${prevGenerationId} failed: ${String(e)}`,
      );
    });
  }
  setAiGenerationId((prev) => new Map(prev).set(stepId, generationId));
  setAiLastGenerationId((prev) => new Map(prev).set(stepId, generationId));
  setAiStepError((prev) => {
    const next = new Map(prev);
    next.delete(stepId);
    return next;
  });
  setAiBusy(stepId, true);
  log.info(
    `generateAi: step=${stepId}, generation=${generationId}, ${entries.length} entries, ` +
      `profile=${profile.id}, model=${profile.model}, hasKey=${profile.hasKey}, ` +
      `chunkSize=${profile.chunkSize}, concurrency=${profile.concurrency}, ` +
      `timeoutSecs=${profile.timeoutSecs}, maxLen=${profile.maxLen}`,
  );
  log.trace(`generateAi: prompt=${prompt}`);

  const isLive = () => aiGenerationId().get(stepId) === generationId;

  try {
    const report = await ipc.aiGenerate(prompt, entries, generationId);
    if (!isLive()) {
      log.debug(`generateAi: discarding stale result for generation=${generationId}`);
      return;
    }
    setStepResults(stepId, report.results);
    log.info(
      `generateAi done: step=${stepId}, generation=${generationId}, ${report.results.length} result(s), ` +
        `failed_chunks=${report.failedChunks}/${report.totalChunks}`,
    );
    if (report.warning) {
      log.warn(`generateAi partial: step=${stepId}, generation=${generationId}: ${report.warning}`);
    }
    const stems = new Map(entries.map((e) => [e.id, e.stem]));
    const previewLimit = 50;
    for (const r of report.results.slice(0, previewLimit)) {
      const old = stems.get(r.id) ?? "?";
      log.debug(`generateAi rename: ${r.id} "${old}" -> "${r.newName}"`);
    }
    if (report.results.length > previewLimit) {
      log.debug(`generateAi rename: … and ${report.results.length - previewLimit} more`);
    }
    const resultIds = new Set(report.results.map((r) => r.id));
    const missing = entries.filter((e) => !resultIds.has(e.id));
    if (missing.length > 0) {
      log.debug(
        `generateAi: ${missing.length} file(s) got no suggestion (pipeline keeps original name)`,
      );
      const missingLimit = 20;
      for (const e of missing.slice(0, missingLimit)) {
        log.debug(`generateAi no suggestion: ${e.id} "${e.stem}"`);
      }
      if (missing.length > missingLimit) {
        log.debug(`generateAi no suggestion: … and ${missing.length - missingLimit} more`);
      }
    }
    const changed = report.results.filter((r) => stems.get(r.id) !== r.newName).length;
    const unchanged = report.results.length - changed;
    log.debug(`generateAi quality: ${changed} changed, ${unchanged} unchanged name(s)`);
    if (report.warning) {
      setAiStepError((prev) => new Map(prev).set(stepId, report.warning!));
    }
    setNotice(report.warning ?? `AI suggested ${report.results.length} name(s).`);
  } catch (e) {
    if (!isLive()) {
      log.debug(`generateAi: ignoring error for cancelled generation=${generationId}`);
      return;
    }
    log.error(`generateAi failed: ${String(e)}`);
    setAiStepError((prev) => new Map(prev).set(stepId, String(e)));
    setNotice(`AI request failed: ${String(e)}`);
  } finally {
    if (isLive()) {
      setAiBusy(stepId, false);
    }
    // The backend persists this generation to AI History regardless of success/failure —
    // refresh the list, and if its Details dialog happens to be open (opened while the
    // generation was still in flight, so the first fetch found nothing yet), retry now.
    void refreshAiHistory();
    if (aiDetailOpenId() === generationId) {
      void fetchAiDetail(generationId);
    }
  }
}

export function cancelAi(stepId: string): void {
  const generationId = aiGenerationId().get(stepId);
  setAiGenerationId((prev) => {
    const next = new Map(prev);
    next.delete(stepId);
    return next;
  });
  setAiBusy(stepId, false);
  log.info(
    `cancelAi: step=${stepId}${generationId ? `, generation=${generationId}` : ""}`,
  );
  if (generationId) {
    void ipc.cancelAiGenerate(generationId).catch((e) => {
      log.debug(`cancelAi: best-effort cancel of generation=${generationId} failed: ${String(e)}`);
    });
  }
}

// ---- AI History (persistent request/response log; separate from rename history) -------

const [aiHistory, setAiHistory] = createSignal<AiGenerationSummary[]>([]);
const [aiDetailOpenId, setAiDetailOpenId] = createSignal<string | null>(null);
const [aiDetail, setAiDetail] = createSignal<Map<string, AiGenerationDetail>>(new Map());
const [aiDetailLoading, setAiDetailLoading] = createSignal<Set<string>>(new Set());
export { aiHistory, aiDetailOpenId, aiDetail };

export function isAiDetailLoading(id: string): boolean {
  return aiDetailLoading().has(id);
}

export async function refreshAiHistory(): Promise<void> {
  try {
    setAiHistory(await ipc.listAiGenerations());
  } catch (e) {
    log.error(`refreshAiHistory failed: ${String(e)}`);
    setNotice(`Could not load AI history: ${String(e)}`);
  }
}

async function fetchAiDetail(id: string): Promise<void> {
  setAiDetailLoading((prev) => new Set(prev).add(id));
  try {
    const detail = await ipc.getAiGeneration(id);
    if (detail) {
      setAiDetail((prev) => new Map(prev).set(id, detail));
    }
  } catch (e) {
    log.error(`fetchAiDetail failed: ${String(e)}`);
    setNotice(`Could not load AI request detail: ${String(e)}`);
  } finally {
    setAiDetailLoading((prev) => {
      const next = new Set(prev);
      next.delete(id);
      return next;
    });
  }
}

/** Opens the shared AI request detail dialog for `id`, fetching it if not already cached — a
 * generation still in flight won't have a row yet, so the dialog shows a "still running" state
 * until `generateAi`'s completion refetches it (see the `finally` block above). */
export function openAiDetail(id: string): void {
  setAiDetailOpenId(id);
  if (!aiDetail().has(id)) void fetchAiDetail(id);
}

export function closeAiDetail(): void {
  setAiDetailOpenId(null);
}

// ---- Settings / providers ---------------------------------------------------------------

const [settings, setSettings] = createSignal<SettingsState>({
  profiles: [],
  activeProfileId: null,
  debugLogging: false,
  mockAi: { enabled: false, latencyMs: 500, failRate: 0, transform: "suffix" },
});
export { settings };

export function activeProfile(): ProviderProfile | undefined {
  const s = settings();
  return s.profiles.find((p) => p.id === s.activeProfileId);
}

export async function loadSettings(): Promise<void> {
  try {
    setSettings(await ipc.getSettings());
  } catch (e) {
    log.error(`loadSettings failed: ${String(e)}`);
    setNotice(`Could not load settings: ${String(e)}`);
  }
}

export async function upsertProfile(profile: ProviderProfile): Promise<void> {
  try {
    await ipc.upsertProfile(profile);
    log.debug(`upsertProfile: id=${profile.id}, label=${profile.label}, model=${profile.model}`);
    await loadSettings();
  } catch (e) {
    log.error(`upsertProfile failed: ${String(e)}`);
    setNotice(`Could not save profile: ${String(e)}`);
    throw e;
  }
}

export async function deleteProfile(id: string): Promise<void> {
  try {
    await ipc.deleteProfile(id);
    log.debug(`deleteProfile: id=${id}`);
    await loadSettings();
  } catch (e) {
    log.error(`deleteProfile failed: ${String(e)}`);
    setNotice(`Could not delete profile: ${String(e)}`);
    throw e;
  }
}

export async function setActiveProfile(id: string): Promise<void> {
  try {
    await ipc.setActiveProfile(id);
    log.debug(`setActiveProfile: id=${id}`);
    await loadSettings();
  } catch (e) {
    log.error(`setActiveProfile failed: ${String(e)}`);
    setNotice(`Could not set active profile: ${String(e)}`);
    throw e;
  }
}

export async function saveApiKey(profileId: string, key: string): Promise<void> {
  // REDACTION: never log `key` — only the profile id.
  log.debug(`saveApiKey: profileId=${profileId}`);
  try {
    await ipc.setApiKey(profileId, key);
    await loadSettings();
  } catch (e) {
    log.error(`saveApiKey failed: ${String(e)}`);
    setNotice(`Could not save API key: ${String(e)}`);
    throw e;
  }
}

export async function clearApiKey(profileId: string): Promise<void> {
  log.debug(`clearApiKey: profileId=${profileId}`);
  try {
    await ipc.clearApiKey(profileId);
    await loadSettings();
  } catch (e) {
    log.error(`clearApiKey failed: ${String(e)}`);
    setNotice(`Could not clear API key: ${String(e)}`);
    throw e;
  }
}

export async function testConnection(profileId: string): Promise<string> {
  log.info(`testConnection: profileId=${profileId}`);
  try {
    const result = await ipc.testConnection(profileId);
    log.info(`testConnection: profileId=${profileId} ok`);
    return result;
  } catch (e) {
    log.warn(`testConnection: profileId=${profileId} failed: ${String(e)}`);
    throw e;
  }
}

export async function setDebugLogging(enabled: boolean): Promise<void> {
  try {
    await ipc.setDebugLogging(enabled);
    log.info(`setDebugLogging: ${enabled ? "enabled" : "disabled"}`);
    await loadSettings();
  } catch (e) {
    log.error(`setDebugLogging failed: ${String(e)}`);
    setNotice(`Could not update debug logging: ${String(e)}`);
    throw e;
  }
}

// ---- Dev menu: Mock AI ------------------------------------------------------------------

export async function setMockAiConfig(config: MockAiConfig): Promise<void> {
  try {
    await ipc.setMockAiConfig(config);
    log.info(
      `setMockAiConfig: enabled=${config.enabled}, latencyMs=${config.latencyMs}, ` +
        `failRate=${config.failRate}, transform=${config.transform}`,
    );
    await loadSettings();
  } catch (e) {
    log.error(`setMockAiConfig failed: ${String(e)}`);
    setNotice(`Could not update Mock AI config: ${String(e)}`);
    throw e;
  }
}
