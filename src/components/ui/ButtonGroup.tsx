import type { ButtonHTMLAttributes, ReactNode } from "react";

export type ButtonGroupProps = {
  children: ReactNode;
  align?: "start" | "end" | "stretch";
};

export function ButtonGroup({ children, align = "end" }: ButtonGroupProps) {
  return <div className={`ui-button-group is-${align}`}>{children}</div>;
}

export type UiButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "danger";
  size?: "sm" | "md";
};

export function UiButton({ variant = "secondary", size = "md", className = "", ...props }: UiButtonProps) {
  const classes = ["ui-button", `is-${variant}`, `is-${size}`, className].filter(Boolean).join(" ");
  return <button className={classes} {...props} />;
}
