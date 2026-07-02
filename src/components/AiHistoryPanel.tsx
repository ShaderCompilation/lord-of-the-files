import { For, Show } from "solid-js";

import * as s from "../store";
import { aiStatusBadgeVariant, aiStatusLabel } from "../lib/aiHistoryStatus";
import { Badge, Button, Overlay } from "./common";

export function AiHistoryPanel(props: { open: boolean; onClose: () => void }) {
  return (
    <Show when={props.open}>
      <Overlay ariaLabel="Close AI history" onClick={props.onClose} />
      <aside class="history-panel">
        <div class="history-head">
          <h2>AI History</h2>
          <Button variant="icon" onClick={props.onClose} title="Close">
            ✕
          </Button>
        </div>
        <Show
          when={s.aiHistory().length > 0}
          fallback={<p class="muted hint">No AI requests yet.</p>}
        >
          <ul class="history-list">
            <For each={s.aiHistory()}>
              {(gen) => (
                <li class="history-item">
                  <button
                    type="button"
                    class="history-row"
                    onClick={() => s.openAiDetail(gen.id)}
                  >
                    <div class="history-info">
                      <span class="history-summary">{gen.instruction || "(no instruction)"}</span>
                      <span class="muted small">
                        {new Date(gen.createdAt).toLocaleString()} · {gen.profileLabel} (
                        {gen.model}) · {gen.entryCount} file{gen.entryCount === 1 ? "" : "s"} ·{" "}
                        <Badge variant={aiStatusBadgeVariant(gen.status)}>
                          {aiStatusLabel(gen.status)}
                        </Badge>
                        <Show when={gen.mock}> · mocked</Show>
                      </span>
                    </div>
                  </button>
                </li>
              )}
            </For>
          </ul>
        </Show>
      </aside>
    </Show>
  );
}
