import { Checkbox, TextField } from "../common";
import type { SetFn, Variant } from "./types";

export function RegexFields(props: { step: Variant<"regex">; set: SetFn }) {
  return (
    <>
      <TextField
        label="Pattern"
        mono
        value={props.step.pattern}
        onInput={(v) => props.set({ pattern: v })}
        placeholder="(\\d+)"
      />
      <TextField
        label="Replacement"
        mono
        value={props.step.replacement}
        onInput={(v) => props.set({ replacement: v })}
        placeholder="${1}"
      />
      <Checkbox checked={props.step.ignoreCase} onChange={(v) => props.set({ ignoreCase: v })}>
        Ignore case
      </Checkbox>
      <Checkbox checked={props.step.multiline} onChange={(v) => props.set({ multiline: v })}>
        Multiline
      </Checkbox>
    </>
  );
}
