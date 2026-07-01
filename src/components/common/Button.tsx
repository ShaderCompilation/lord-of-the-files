import type { JSX } from "solid-js";

export function Button(props: {
  variant?: "primary" | "ghost" | "icon" | "chip";
  danger?: boolean;
  small?: boolean;
  class?: string;
  onClick?: (e: MouseEvent) => void;
  disabled?: boolean;
  title?: string;
  children: JSX.Element;
}) {
  const classes = () =>
    [props.variant, props.danger && "danger", props.small && "small", props.class]
      .filter(Boolean)
      .join(" ");

  return (
    <button
      type="button"
      class={classes()}
      onClick={props.onClick}
      disabled={props.disabled}
      title={props.title}
    >
      {props.children}
    </button>
  );
}
