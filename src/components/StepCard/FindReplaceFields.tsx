import { Checkbox, TextField } from "../common";
import type { SetFn, Variant } from "./types";

export function FindReplaceFields(props: { step: Variant<"findReplace">; set: SetFn }) {
  return (
    <>
      <TextField label="Find" value={props.step.find} onInput={(v) => props.set({ find: v })} />
      <TextField
        label="Replace"
        value={props.step.replace}
        onInput={(v) => props.set({ replace: v })}
      />
      <Checkbox
        checked={props.step.caseSensitive}
        onChange={(v) => props.set({ caseSensitive: v })}
      >
        Case sensitive
      </Checkbox>
      <Checkbox
        checked={props.step.allOccurrences}
        onChange={(v) => props.set({ allOccurrences: v })}
      >
        All occurrences
      </Checkbox>
    </>
  );
}
