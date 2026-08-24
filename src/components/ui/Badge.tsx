import type { HTMLAttributes, ReactNode } from "react";

export type BadgeTone = "neutral" | "accent" | "success" | "warning" | "danger";

export type BadgeProps = HTMLAttributes<HTMLSpanElement> & {
  children: ReactNode;
  tone?: BadgeTone;
};

export function Badge({ children, tone = "neutral", className = "", ...props }: BadgeProps) {
  const classes = ["ui-badge", `is-${tone}`, className].filter(Boolean).join(" ");
  return <span className={classes} {...props}>{children}</span>;
}
