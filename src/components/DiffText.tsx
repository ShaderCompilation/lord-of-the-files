import { For, createMemo } from "solid-js";

import { diffChars } from "../lib/diff";

/** Renders `next` with inline char-level diff vs `original` (inserts green, deletes red). */
export function DiffText(props: { original: string; next: string }) {
  const segments = createMemo(() => diffChars(props.original, props.next));
  return (
    <span class="diff">
      <For each={segments()}>
        {(seg) => <span class={`diff-${seg.kind}`}>{seg.text}</span>}
      </For>
    </span>
  );
}
