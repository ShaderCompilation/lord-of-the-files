// Character-level diff for the preview's "new name" column. Computed lazily, only for
// rows currently visible in the virtualized list.

import fastDiff from "fast-diff";

export type DiffKind = "equal" | "insert" | "delete";

export interface DiffSeg {
  kind: DiffKind;
  text: string;
}

/** Diff `original` -> `next`, returning segments to render (inserts green, deletes red). */
export function diffChars(original: string, next: string): DiffSeg[] {
  return fastDiff(original, next).map(([op, text]) => ({
    kind: op === fastDiff.INSERT ? "insert" : op === fastDiff.DELETE ? "delete" : "equal",
    text,
  }));
}
