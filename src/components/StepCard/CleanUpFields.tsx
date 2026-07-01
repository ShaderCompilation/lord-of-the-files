import { Checkbox, SelectField } from "../common";
import type { SetFn, Variant } from "./types";

export function CleanUpFields(props: { step: Variant<"cleanUp">; set: SetFn }) {
  const spaceValue = () =>
    props.step.spacesTo === null ? "none" : props.step.spacesTo === "" ? "remove" : props.step.spacesTo;
  const onSpace = (v: string) =>
    props.set({ spacesTo: v === "none" ? null : v === "remove" ? "" : v });

  return (
    <>
      <Checkbox checked={props.step.trim} onChange={(v) => props.set({ trim: v })}>
        Trim ends
      </Checkbox>
      <Checkbox
        checked={props.step.collapseWhitespace}
        onChange={(v) => props.set({ collapseWhitespace: v })}
      >
        Collapse whitespace
      </Checkbox>
      <Checkbox
        checked={props.step.stripDiacritics}
        onChange={(v) => props.set({ stripDiacritics: v })}
      >
        Strip diacritics
      </Checkbox>
      <SelectField label="Spaces →" value={spaceValue()} onChange={onSpace}>
        <option value="none">Keep</option>
        <option value="-">Dash (-)</option>
        <option value="_">Underscore (_)</option>
        <option value="remove">Remove</option>
      </SelectField>
    </>
  );
}
