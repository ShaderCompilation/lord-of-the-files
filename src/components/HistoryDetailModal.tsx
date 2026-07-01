import { For, Show, createMemo } from "solid-js";

import * as s from "../store";
import {
  basename,
  checkBadgeVariant,
  checkLabel,
  checkTooltip,
  cleanError,
  opBadgeVariant,
  opStatusLabel,
  rowBadgeVariant,
  rowTooltip,
} from "../lib/historyStatus";
import { Badge, Button, Overlay } from "./common";
import { DiffText } from "./DiffText";

export function HistoryDetailModal() {
  const op = createMemo(() => s.history().find((o) => o.id === s.historyDetailOpId()));

  return (
    <Show when={op()}>
      {(o) => {
        const pending = createMemo(() =>
          s.pendingAction()?.opId === o().id ? s.pendingAction() : null,
        );
        const files = createMemo(() => s.opFiles().get(o().id) ?? []);
        const errors = createMemo(() => s.opErrors().get(o().id));
        const busy = () => s.pendingLoading() || s.confirmBusy();

        return (
          <>
            <Overlay ariaLabel="Close operation details" onClick={() => s.closeHistoryDetail()} />
            <div class="history-modal" role="dialog" aria-label="Operation details">
              <div class="history-modal-head">
                <div class="history-info">
                  <span class="history-summary">{o().summary}</span>
                  <span class="muted small">
                    {new Date(o().createdAt).toLocaleString()} ·{" "}
                    <Badge variant={opBadgeVariant(o().status)}>{opStatusLabel(o().status)}</Badge>
                  </span>
                </div>
                <Button variant="icon" onClick={() => s.closeHistoryDetail()} title="Close">
                  ✕
                </Button>
              </div>

              <Show when={errors() && errors()!.length > 0}>
                <ul class="history-errors">
                  <For each={errors()}>
                    {(f) => (
                      <li class="history-error-row">
                        <span class="mono small">{basename(f.path)}</span>: {cleanError(f.error)}
                      </li>
                    )}
                  </For>
                </ul>
              </Show>

              <Show
                when={pending()}
                fallback={
                  <>
                    <ul class="history-file-list history-modal-list">
                      <Show
                        when={!s.isOpFilesLoading(o().id)}
                        fallback={<li class="muted small">Loading…</li>}
                      >
                        <For each={files()}>
                          {(f) => (
                            <li class="history-file-row">
                              <DiffText original={f.oldPath} next={f.newPath} />
                              <Badge variant={rowBadgeVariant(f.status)} title={rowTooltip(f.status)}>
                                {f.status}
                              </Badge>
                            </li>
                          )}
                        </For>
                      </Show>
                    </ul>
                    <div class="history-confirm-actions">
                      <Show
                        when={o().status !== "undone"}
                        fallback={
                          <Button small disabled={busy()} onClick={() => s.requestRedo(o().id)}>
                            Redo
                          </Button>
                        }
                      >
                        <Button small disabled={busy()} onClick={() => s.requestUndo(o().id)}>
                          Undo
                        </Button>
                      </Show>
                    </div>
                  </>
                }
              >
                {(p) => (
                  <>
                    <p class="small">
                      {p().direction === "undo" ? "Undo" : "Redo"} will affect {p().checks.length}{" "}
                      file(s):
                    </p>
                    <ul class="history-file-list history-modal-list">
                      <For each={p().checks}>
                        {(c) => (
                          <li class="history-file-row">
                            <DiffText original={c.oldPath} next={c.newPath} />
                            <Badge variant={checkBadgeVariant(c.status)} title={checkTooltip(c.status)}>
                              {checkLabel(c.status)}
                            </Badge>
                          </li>
                        )}
                      </For>
                    </ul>
                    <div class="history-confirm-actions">
                      <Button small onClick={() => s.cancelPendingAction()} disabled={s.confirmBusy()}>
                        Cancel
                      </Button>
                      <Button
                        variant="primary"
                        small
                        onClick={() => s.confirmPendingAction()}
                        disabled={s.confirmBusy()}
                      >
                        {s.confirmBusy() ? "Working…" : "Confirm"}
                      </Button>
                    </div>
                  </>
                )}
              </Show>
            </div>
          </>
        );
      }}
    </Show>
  );
}
