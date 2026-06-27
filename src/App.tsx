import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Show, createEffect, createSignal, onCleanup, onMount } from "solid-js";

import "./App.css";
import * as s from "./store";
import { Toolbar } from "./components/Toolbar";
import { FileTable } from "./components/FileTable";
import { PipelineEditor } from "./components/PipelineEditor";
import { HistoryPanel } from "./components/HistoryPanel";

export default function App() {
  const [historyOpen, setHistoryOpen] = createSignal(false);

  // Debounced live preview: recompute whenever files or the pipeline change.
  let timer: ReturnType<typeof setTimeout> | undefined;
  createEffect(() => {
    s.files();
    s.pipelineVersion();
    clearTimeout(timer);
    timer = setTimeout(() => void s.runPreview(), 150);
  });
  onCleanup(() => clearTimeout(timer));

  onMount(async () => {
    await s.refreshHistory();
    const unlisten = await getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "drop") {
        void s.addPaths(event.payload.paths);
      }
    });
    onCleanup(unlisten);
  });

  return (
    <div class="app">
      <Toolbar onToggleHistory={() => setHistoryOpen((v) => !v)} />

      <Show when={s.notice()}>
        <div class="notice">
          <span>{s.notice()}</span>
          <button type="button" class="notice-close" onClick={() => s.setNotice(null)} title="Dismiss">
            ✕
          </button>
        </div>
      </Show>

      <main class="main">
        <FileTable />
        <PipelineEditor />
      </main>

      <HistoryPanel open={historyOpen()} onClose={() => setHistoryOpen(false)} />
    </div>
  );
}
