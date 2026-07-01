import { For, Show } from "solid-js";

import * as s from "../store";
import { opBadgeVariant, opStatusLabel } from "../lib/historyStatus";
import { Badge, Button, Overlay } from "./common";
import { HistoryDetailModal } from "./HistoryDetailModal";

export function HistoryPanel(props: { open: boolean; onClose: () => void }) {
  return (
    <Show when={props.open}>
      <Overlay ariaLabel="Close history" onClick={props.onClose} />
      <aside class="history-panel">
        <div class="history-head">
          <h2>History</h2>
          <Button variant="icon" onClick={props.onClose} title="Close">
            ✕
          </Button>
        </div>
        <Show
          when={s.history().length > 0}
          fallback={<p class="muted hint">No operations yet.</p>}
        >
          <ul class="history-list">
            <For each={s.history()}>
              {(op) => {
                const busy = () => s.pendingLoading() || s.confirmBusy();
                return (
                  <li class="history-item">
                    <button
                      type="button"
                      class="history-row"
                      onClick={() => s.openHistoryDetail(op.id)}
                    >
                      <div class="history-info">
                        <span class="history-summary">{op.summary}</span>
                        <span class="muted small">
                          {new Date(op.createdAt).toLocaleString()} ·{" "}
                          <Badge variant={opBadgeVariant(op.status)}>
                            {opStatusLabel(op.status)}
                          </Badge>
                        </span>
                      </div>
                    </button>
                    <Show
                      when={op.status !== "undone"}
                      fallback={
                        <Button
                          variant="ghost"
                          small
                          disabled={busy()}
                          onClick={() => s.requestRedo(op.id)}
                        >
                          Redo
                        </Button>
                      }
                    >
                      <Button
                        variant="ghost"
                        small
                        disabled={busy()}
                        onClick={() => s.requestUndo(op.id)}
                      >
                        Undo
                      </Button>
                    </Show>
                  </li>
                );
              }}
            </For>
          </ul>
        </Show>
      </aside>
      <HistoryDetailModal />
    </Show>
  );
}
