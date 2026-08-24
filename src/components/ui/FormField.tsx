import { cloneElement, isValidElement, useId, type ReactElement, type ReactNode } from "react";

export type FormFieldProps = {
  label: string;
  children: ReactNode;
  hint?: ReactNode;
  required?: boolean;
  htmlFor?: string;
};

export function FormField({ label, children, hint, required = false, htmlFor }: FormFieldProps) {
  const generatedId = useId();
  const controlId = htmlFor ?? generatedId;
  const hintId = hint ? `${controlId}-hint` : undefined;
  let control = children;
  if (isValidElement(children)) {
    const child = children as ReactElement<{ id?: string; "aria-describedby"?: string }>;
    control = cloneElement(child, {
      id: child.props.id ?? controlId,
      "aria-describedby": child.props["aria-describedby"] ?? hintId,
    });
  }

  return (
    <div className="ui-form-field">
      <label htmlFor={controlId}>
        {label}
        {required && <span className="ui-form-required" aria-hidden>＊</span>}
      </label>
      <div className="ui-form-control">{control}</div>
      {hint && <p id={hintId} className="ui-form-hint">{hint}</p>}
    </div>
  );
}
