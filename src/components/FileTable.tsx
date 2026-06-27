import { createVirtualizer } from "@tanstack/solid-virtual";
import { For, Show, createMemo, createSignal } from "solid-js";

import * as s from "../store";
import type { PreviewRow, RowStatus } from "../lib/types";
import { DiffText } from "./DiffText";

const STATUS_LABEL: Record<RowStatus, string> = {
  unchanged: "—",
  changed: "Changed",
  conflict: "Conflict",
  invalid: "Invalid",
};

export function FileTable() {
  const [onlyChanged, setOnlyChanged] = createSignal(false);
  let parentRef: HTMLDivElement | undefined;

  const rows = createMemo<PreviewRow[]>(() => {
    const all = s.preview().rows;
    return onlyChanged() ? all.filter((r) => r.status !== "unchanged") : all;
  });

  const virtualizer = createVirtualizer({
    get count() {
      return rows().length;
    },
    getScrollElement: () => parentRef ?? null,
    estimateSize: () => 34,
    overscan: 16,
  });

  return (
    <div class="filetable">
      <div class="filetable-head">
        <div class="th col-include" />
        <div class="th">Original</div>
        <div class="th">New name</div>
        <div class="th col-status">Status</div>
      </div>

      <Show
        when={s.files().length > 0}
        fallback={
          <div class="empty">
            <p>Drop files or folders here, or use “Add files” / “Add folder”.</p>
          </div>
        }
      >
        <div class="filetable-toolbar">
          <label class="check">
            <input
              type="checkbox"
              checked={onlyChanged()}
              onChange={(e) => setOnlyChanged(e.currentTarget.checked)}
            />
            Show only changed
          </label>
          <span class="muted">{s.files().length} file(s)</span>
        </div>

        <div class="filetable-body" ref={parentRef}>
          <div
            class="filetable-rows"
            style={{ height: `${virtualizer.getTotalSize()}px` }}
          >
            <For each={virtualizer.getVirtualItems()}>
              {(vi) => {
                const row = () => rows()[vi.index];
                return (
                  <div
                    class="tr"
                    classList={{
                      excluded: s.isExcluded(row().id),
                      [`status-${row().status}`]: true,
                    }}
                    style={{
                      transform: `translateY(${vi.start}px)`,
                      height: `${vi.size}px`,
                    }}
                  >
                    <div class="td col-include">
                      <input
                        type="checkbox"
                        checked={!s.isExcluded(row().id)}
                        disabled={row().status === "unchanged"}
                        onChange={() => s.toggleExclude(row().id)}
                        title="Include in rename"
                      />
                    </div>
                    <div class="td original" title={row().original}>
                      {row().original}
                    </div>
                    <div class="td newname" title={row().newName}>
                      <Show
                        when={row().status !== "unchanged"}
                        fallback={<span class="muted">{row().newName}</span>}
                      >
                        <DiffText original={row().original} next={row().newName} />
                      </Show>
                    </div>
                    <div class="td col-status">
                      <span
                        class={`badge badge-${row().status}`}
                        title={row().message ?? ""}
                      >
                        {STATUS_LABEL[row().status]}
                      </span>
                    </div>
                  </div>
                );
              }}
            </For>
          </div>
        </div>
      </Show>
    </div>
  );
}
