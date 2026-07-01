import type { JSX } from "solid-js";

export function Checkbox(props: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  class?: string;
  title?: string;
  type?: "checkbox" | "radio";
  name?: string;
  children?: JSX.Element;
}) {
  return (
    <label class={props.class ? `check ${props.class}` : "check"} title={props.title}>
      <input
        type={props.type ?? "checkbox"}
        name={props.name}
        checked={props.checked}
        onChange={(e) => props.onChange(e.currentTarget.checked)}
      />
      {props.children}
    </label>
  );
}
