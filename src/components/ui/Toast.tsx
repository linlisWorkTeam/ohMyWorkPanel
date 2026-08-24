import type { ReactNode } from "react";

export type ToastProps = {
  children: ReactNode;
  tone?: "neutral" | "success" | "warning" | "danger";
  onClose?: () => void;
  stacked?: boolean;
};

export function Toast({ children, tone = "neutral", onClose, stacked = false }: ToastProps) {
  const urgent = tone === "danger";
  return (
    <div
      className={`error-toast ui-toast is-${tone}${stacked ? " is-stacked" : ""}`}
      role={urgent ? "alert" : "status"}
      aria-live={urgent ? "assertive" : "polite"}
    >
      <span>{children}</span>
      {onClose && (
        <button type="button" aria-label="关闭提示" onClick={onClose}>×</button>
      )}
    </div>
  );
}
