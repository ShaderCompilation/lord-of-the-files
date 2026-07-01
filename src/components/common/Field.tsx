import type { JSX } from "solid-js";

export function Field(props: { label: string; class?: string; children: JSX.Element }) {
  return (
    <label class={props.class ? `field ${props.class}` : "field"}>
      {props.label}
      {props.children}
    </label>
  );
}

export function TextField(props: {
  label: string;
  value: string | number;
  onInput: (value: string) => void;
  type?: "text" | "number";
  mono?: boolean;
  placeholder?: string;
  min?: string | number;
  class?: string;
}) {
  return (
    <Field label={props.label} class={props.class}>
      <input
        type={props.type ?? "text"}
        class={props.mono ? "mono" : undefined}
        value={props.value}
        min={props.min}
        placeholder={props.placeholder}
        onInput={(e) => props.onInput(e.currentTarget.value)}
      />
    </Field>
  );
}

export function SelectField(props: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  class?: string;
  children: JSX.Element;
}) {
  return (
    <Field label={props.label} class={props.class}>
      <select value={props.value} onChange={(e) => props.onChange(e.currentTarget.value)}>
        {props.children}
      </select>
    </Field>
  );
}

export function TextareaField(props: {
  label: string;
  value: string;
  onInput: (value: string) => void;
  rows?: number;
  placeholder?: string;
}) {
  return (
    <Field label={props.label}>
      <textarea
        rows={props.rows ?? 3}
        value={props.value}
        placeholder={props.placeholder}
        onInput={(e) => props.onInput(e.currentTarget.value)}
      />
    </Field>
  );
}
