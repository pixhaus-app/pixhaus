import { splitProps, type JSX } from "solid-js";

type Variant = "primary" | "ghost" | "danger";
type Size = "sm" | "md" | "lg";

interface ButtonProps extends Omit<JSX.ButtonHTMLAttributes<HTMLButtonElement>, "type"> {
  variant?: Variant;
  size?: Size;
  loading?: boolean;
  type?: "button" | "submit" | "reset";
}

export function Button(props: ButtonProps) {
  const [local, rest] = splitProps(props, [
    "variant",
    "size",
    "loading",
    "disabled",
    "class",
    "type",
    "children",
  ]);

  const variant = () => local.variant ?? "primary";
  const size = () => local.size ?? "md";
  const isLoading = () => local.loading === true;
  const isDisabled = () => isLoading() || local.disabled === true;

  const classes = () => {
    const parts = ["btn", `btn--${variant()}`];
    if (size() !== "md") parts.push(`btn--${size()}`);
    if (isLoading()) parts.push("btn--loading");
    if (local.class) parts.push(local.class);
    return parts.join(" ");
  };

  return (
    <button
      {...rest}
      type={local.type ?? "button"}
      class={classes()}
      disabled={isDisabled()}
      aria-busy={isLoading() || undefined}
    >
      {local.children}
    </button>
  );
}
