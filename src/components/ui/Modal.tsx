import { useEffect, useId, useRef, type ReactNode } from "react";

export type ModalProps = {
  title: string;
  children: ReactNode;
  onClose: () => void;
  closeLabel?: string;
};

export function Modal({ title, children, onClose, closeLabel = "关闭弹窗" }: ModalProps) {
  const titleId = useId();
  const dialogRef = useRef<HTMLElement | null>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialogRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onCloseRef.current();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      previousFocus?.focus();
    };
  }, []);

  return (
    <div className="modal-backdrop ui-modal-backdrop">
      <section
        ref={dialogRef}
        className="modal ui-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
      >
        <header className="ui-modal-header">
          <h2 id={titleId}>{title}</h2>
          <button className="icon-button ui-modal-close" type="button" aria-label={closeLabel} onClick={onClose}>
            ×
          </button>
        </header>
        <div className="ui-modal-body">{children}</div>
      </section>
    </div>
  );
}
