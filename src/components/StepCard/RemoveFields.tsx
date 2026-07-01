import { Show } from "solid-js";

import { SelectField, TextField } from "../common";
import type { SetFn, Variant } from "./types";

export function RemoveFields(props: { step: Variant<"remove">; set: SetFn }) {
  return (
    <>
      <SelectField label="From" value={props.step.from} onChange={(v) => props.set({ from: v })}>
        <option value="start">Start</option>
        <option value="end">End</option>
        <option value="index">Index</option>
      </SelectField>
      <TextField
        label="Count"
        type="number"
        min="0"
        value={props.step.count}
        onInput={(v) => props.set({ count: Number(v) })}
      />
      <Show when={props.step.from === "index"}>
        <TextField
          label="Index"
          type="number"
          min="0"
          value={props.step.index}
          onInput={(v) => props.set({ index: Number(v) })}
        />
      </Show>
    </>
  );
}
