import { createVirtualizer } from "@tanstack/solid-virtual";
import { For, Show, createMemo } from "solid-js";

import * as s from "../store";
import type { PreviewRow, RowStatus } from "../lib/types";
import { Badge, Button } from "./common";
import { DiffText } from "./DiffText";

const STATUS_LABEL: Record<RowStatus, string> = {
  unchanged: "—",
  changed: "Changed",
  conflict: "Conflict",
  invalid: "Invalid",
};

function matchesFilter(status: RowStatus, filter: s.TableFilter): boolean {
  switch (filter) {
    case "all":
      return true;
    case "changed":
      return status === "changed";
    case "conflict":
      return status === "conflict" || status === "invalid";
    case "unchanged":
      return status === "unchanged";
  }
}

export function FileTable() {
  let parentRef: HTMLDivElement | undefined;

  const rows = createMemo<PreviewRow[]>(() => {
    const filter = s.tableFilter();
    return s.preview().rows.filter((r) => matchesFilter(r.status, filter));
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
      <Show
        when={s.files().length > 0}
        fallback={
          <div class="dropzone">
            <div class="dropzone-icon">🗂️</div>
            <div class="dropzone-step">Step 1</div>
            <p class="dropzone-title">Drop files or folders here</p>
            <p class="muted">or</p>
            <Button variant="primary" onClick={s.pickFiles}>
              Add files
            </Button>
            <p class="muted small dropzone-hint">
              Then build a recipe or use AI&nbsp;✦ on the right.
            </p>
          </div>
        }
      >
        <FilterBar />

        <div class="filetable-head">
          <div class="th col-include" />
          <div class="th">Original</div>
          <div class="th col-arrow" />
          <div class="th">New name</div>
          <div class="th col-status">Status</div>
        </div>

        <div class="filetable-body" ref={parentRef}>
          <div class="filetable-rows" style={{ height: `${virtualizer.getTotalSize()}px` }}>
            <For each={virtualizer.getVirtualItems()}>
              {(vi) => {
                const row = () => rows()[vi.index];
                const isConflict = () =>
                  row().status === "conflict" || row().status === "invalid";
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
                    <div class="td col-arrow" classList={{ dim: row().status === "unchanged" }}>
                      →
                    </div>
                    <div class="td newname" title={row().message ?? row().newName}>
                      <Show
                        when={row().status !== "unchanged"}
                        fallback={<span class="muted">{row().newName}</span>}
                      >
                        <DiffText original={row().original} next={row().newName} />
                      </Show>
                      <Show when={isConflict() && row().message}>
                        <span class="conflict-reason"> — {row().message}</span>
                      </Show>
                    </div>
                    <div class="td col-status">
                      <Badge variant={row().status} title={row().message ?? ""}>
                        {STATUS_LABEL[row().status]}
                      </Badge>
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

function FilterBar() {
  const counts = createMemo(() => s.previewCounts());
  return (
    <div class="filetable-toolbar">
      <div class="filter-pills">
        <FilterPill value="all" label="All" count={counts().total} />
        <FilterPill value="changed" label="Changed" count={counts().changed} tone="green" />
        <FilterPill value="conflict" label="Conflicts" count={counts().conflict} tone="red" />
        <FilterPill value="unchanged" label="Unchanged" count={counts().unchanged} />
      </div>
      <span class="muted">{s.files().length} file(s)</span>
    </div>
  );
}

function FilterPill(props: {
  value: s.TableFilter;
  label: string;
  count: number;
  tone?: "green" | "red";
}) {
  const active = () => s.tableFilter() === props.value;
  return (
    <button
      type="button"
      class="filter-pill"
      classList={{
        active: active(),
        [`tone-${props.tone}`]: !!props.tone && props.count > 0,
      }}
      onClick={() => s.setTableFilter(props.value)}
    >
      {props.label}
      <span class="filter-pill-count">{props.count}</span>
    </button>
  );
}
