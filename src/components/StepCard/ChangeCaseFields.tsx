import { SelectField } from "../common";
import type { SetFn, Variant } from "./types";

export function ChangeCaseFields(props: { step: Variant<"changeCase">; set: SetFn }) {
  return (
    <SelectField label="Mode" value={props.step.mode} onChange={(v) => props.set({ mode: v })}>
      <option value="lower">lower case</option>
      <option value="upper">UPPER CASE</option>
      <option value="title">Title Case</option>
      <option value="sentence">Sentence case</option>
      <option value="camel">camelCase</option>
      <option value="snake">snake_case</option>
      <option value="kebab">kebab-case</option>
    </SelectField>
  );
}
