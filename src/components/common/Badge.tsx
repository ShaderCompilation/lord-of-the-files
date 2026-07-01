import type { JSX } from "solid-js";

export function Badge(props: {
  variant?: "changed" | "conflict" | "invalid" | "unchanged";
  title?: string;
  children: JSX.Element;
}) {
  return (
    <span class={props.variant ? `badge badge-${props.variant}` : "badge"} title={props.title}>
      {props.children}
    </span>
  );
}
