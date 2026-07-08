import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  FileEntry,
  PreviewResult,
  SettingsState,
} from "./lib/types";

const ipc = vi.hoisted(() => ({
  scanPaths: vi.fn(),
  computePreview: vi.fn(),
  applyRename: vi.fn(),
  listOperations: vi.fn(),
  getSettings: vi.fn(),
  upsertProfile: vi.fn(),
  setApiKey: vi.fn(),
  setActiveProfile: vi.fn(),
  clearApiKey: vi.fn(),
  testConnection: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("@tauri-apps/plugin-log", () => ({
  trace: vi.fn(),
  debug: vi.fn(),
  info: vi.fn(),
  warn: vi.fn(),
  error: vi.fn(),
}));
vi.mock("./lib/ipc", () => ipc);

function entry(path = "/dir/a.txt"): FileEntry {
  return {
    id: path,
    path,
    parentDir: "/dir",
    stem: "a",
    ext: "txt",
    isDir: false,
    size: 1,
    modified: null,
  };
}

function preview(newName: string): PreviewResult {
  return {
    rows: [
      {
        id: "/dir/a.txt",
        original: "a.txt",
        newName,
        status: "changed",
        message: null,
      },
    ],
    stepErrors: [],
  };
}

function settings(activeProfileId: string | null = null): SettingsState {
  return {
    profiles: [],
    activeProfileId,
    debugLogging: false,
    mockAi: { enabled: false, latencyMs: 500, failRate: 0, transform: "suffix" },
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  ipc.scanPaths.mockResolvedValue([entry()]);
  ipc.computePreview.mockResolvedValue(preview("fresh.txt"));
  ipc.applyRename.mockResolvedValue({
    operationId: "op-1",
    renamed: 1,
    failures: [],
    historyError: null,
  });
  ipc.listOperations.mockResolvedValue([]);
  ipc.getSettings.mockResolvedValue(settings());
  ipc.upsertProfile.mockResolvedValue(undefined);
  ipc.setApiKey.mockResolvedValue(undefined);
  ipc.setActiveProfile.mockResolvedValue(undefined);
  ipc.clearApiKey.mockResolvedValue(undefined);
  ipc.testConnection.mockResolvedValue("ok");
});

describe("preview freshness", () => {
  it("refreshes stale preview before applying renames", async () => {
    const store = await import("./store");
    ipc.computePreview
      .mockResolvedValueOnce(preview("old.txt"))
      .mockResolvedValueOnce(preview("fresh.txt"));

    await store.addPaths(["/dir/a.txt"]);
    await store.runPreview();
    store.addStep("findReplace");

    await store.applyAll();

    expect(ipc.applyRename).toHaveBeenCalledWith([
      { oldPath: "/dir/a.txt", newName: "fresh.txt" },
    ]);
  });

  it("ignores out-of-order preview responses from older requests", async () => {
    const store = await import("./store");
    const first = deferred<PreviewResult>();
    const second = deferred<PreviewResult>();
    ipc.computePreview
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);

    await store.addPaths(["/dir/a.txt"]);
    const oldRequest = store.runPreview();
    store.addStep("findReplace");
    const newRequest = store.runPreview();

    second.resolve(preview("new.txt"));
    await newRequest;
    first.resolve(preview("old.txt"));
    await oldRequest;

    expect(store.preview().rows[0]?.newName).toBe("new.txt");
    expect(store.previewStale()).toBe(false);
  });
});

describe("profile editor failure handling", () => {
  it("returns false when profile persistence fails", async () => {
    const { createProfileEditor } = await import("./components/SettingsPanel/profileEditor");
    ipc.upsertProfile.mockRejectedValueOnce(new Error("invalid url"));
    const editor = createProfileEditor();
    editor.begin(null);
    editor.selectPreset("OpenAI");

    await expect(editor.save()).resolves.toBe(false);
  });

  it("returns false when setting the first active profile fails", async () => {
    const { createProfileEditor } = await import("./components/SettingsPanel/profileEditor");
    ipc.getSettings.mockResolvedValue(settings());
    ipc.setActiveProfile.mockRejectedValueOnce(new Error("cannot activate"));
    const editor = createProfileEditor();
    editor.begin(null);
    editor.selectPreset("OpenAI");

    await expect(editor.save()).resolves.toBe(false);
  });

  it("shows a failed test status when autosave fails before connection test", async () => {
    const { createProfileEditor } = await import("./components/SettingsPanel/profileEditor");
    ipc.upsertProfile.mockRejectedValueOnce(new Error("invalid url"));
    const editor = createProfileEditor();
    editor.begin(null);
    editor.selectPreset("OpenAI");

    await editor.test();

    expect(editor.testStatus()).toEqual({
      ok: false,
      message: "Could not save profile",
    });
    expect(ipc.testConnection).not.toHaveBeenCalled();
  });
});
