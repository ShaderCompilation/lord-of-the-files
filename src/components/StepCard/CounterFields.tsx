import { Checkbox, SelectField, TextField } from "../common";
import type { SetFn, Variant } from "./types";

export function CounterFields(props: { step: Variant<"counter">; set: SetFn }) {
  return (
    <>
      <TextField
        label="Start"
        type="number"
        value={props.step.start}
        onInput={(v) => props.set({ start: Number(v) })}
      />
      <TextField
        label="Step"
        type="number"
        value={props.step.step}
        onInput={(v) => props.set({ step: Number(v) })}
      />
      <TextField
        label="Padding"
        type="number"
        min="0"
        value={props.step.padding}
        onInput={(v) => props.set({ padding: Number(v) })}
      />
      <TextField
        label="Separator"
        value={props.step.separator}
        onInput={(v) => props.set({ separator: v })}
      />
      <SelectField
        label="Position"
        value={props.step.position}
        onChange={(v) => props.set({ position: v })}
      >
        <option value="prefix">Prefix</option>
        <option value="suffix">Suffix</option>
      </SelectField>
      <Checkbox
        checked={props.step.resetPerDirectory}
        onChange={(v) => props.set({ resetPerDirectory: v })}
      >
        Reset per folder
      </Checkbox>
    </>
  );
}
