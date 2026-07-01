import { Show } from "solid-js";

import { SelectField, TextField } from "../common";
import type { SetFn, Variant } from "./types";

export function InsertFields(props: { step: Variant<"insert">; set: SetFn }) {
  return (
    <>
      <TextField label="Text" value={props.step.text} onInput={(v) => props.set({ text: v })} />
      <SelectField
        label="Position"
        value={props.step.position}
        onChange={(v) => props.set({ position: v })}
      >
        <option value="prefix">Prefix</option>
        <option value="suffix">Suffix</option>
        <option value="atIndex">At index</option>
      </SelectField>
      <Show when={props.step.position === "atIndex"}>
        <TextField
          label="Index"
          type="number"
          value={props.step.index}
          onInput={(v) => props.set({ index: Number(v) })}
        />
      </Show>
    </>
  );
}
