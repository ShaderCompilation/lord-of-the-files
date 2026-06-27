import { For, Show } from "solid-js";

import * as s from "../store";

export function HistoryPanel(props: { open: boolean; onClose: () => void }) {
  return (
    <Show when={props.open}>
      <button type="button" class="overlay" aria-label="Close history" onClick={props.onClose} />
      <aside class="history-panel">
        <div class="history-head">
          <h2>History</h2>
          <button type="button" class="icon" onClick={props.onClose} title="Close">
            ✕
          </button>
        </div>
        <Show
          when={s.history().length > 0}
          fallback={<p class="muted hint">No operations yet.</p>}
        >
          <ul class="history-list">
            <For each={s.history()}>
              {(op) => (
                <li class="history-item">
                  <div class="history-info">
                    <span class="history-summary">{op.summary}</span>
                    <span class="muted small">
                      {new Date(op.createdAt).toLocaleString()} · {op.status}
                    </span>
                  </div>
                  <Show
                    when={op.status === "applied"}
                    fallback={
                      <button type="button" class="ghost small" onClick={() => s.redo(op.id)}>
                        Redo
                      </button>
                    }
                  >
                    <button type="button" class="ghost small" onClick={() => s.undo(op.id)}>
                      Undo
                    </button>
                  </Show>
                </li>
              )}
            </For>
          </ul>
        </Show>
      </aside>
    </Show>
  );
}
