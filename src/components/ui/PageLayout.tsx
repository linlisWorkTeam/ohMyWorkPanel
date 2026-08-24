import type { HTMLAttributes, ReactNode } from "react";

/**
 * New user-facing pages compose these primitives instead of defining their own
 * page widths, breakpoints, surfaces, or responsive grids. Visual values stay
 * in semantic --lp-* tokens; container queries keep docked pages adaptive.
 */

function classes(...names: Array<string | false | undefined>) {
  return names.filter(Boolean).join(" ");
}

export type PageShellProps = HTMLAttributes<HTMLDivElement> & {
  children: ReactNode;
  density?: "comfortable" | "compact";
};

export function PageShell({ children, density = "comfortable", className, ...props }: PageShellProps) {
  return <div className={classes("ui-page", `is-${density}`, className)} {...props}>{children}</div>;
}

export type PageHeaderProps = {
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  className?: string;
};

export function PageHeader({ title, description, actions, className }: PageHeaderProps) {
  return (
    <header className={classes("ui-page-header", className)}>
      <div className="ui-page-heading">
        <h1>{title}</h1>
        {description && <p>{description}</p>}
      </div>
      {actions && <div className="ui-page-actions">{actions}</div>}
    </header>
  );
}

export type PageSectionProps = HTMLAttributes<HTMLElement> & {
  children: ReactNode;
  title?: ReactNode;
  description?: ReactNode;
};

export function PageSection({ children, title, description, className, ...props }: PageSectionProps) {
  return (
    <section className={classes("ui-page-section", className)} {...props}>
      {(title || description) && (
        <div className="ui-page-section-head">
          {title && <h2>{title}</h2>}
          {description && <p>{description}</p>}
        </div>
      )}
      {children}
    </section>
  );
}

export type ResponsiveGridProps = HTMLAttributes<HTMLDivElement> & {
  children: ReactNode;
  variant?: "cards" | "form" | "detail";
};

export function ResponsiveGrid({ children, variant = "cards", className, ...props }: ResponsiveGridProps) {
  return <div className={classes("ui-responsive-grid", `is-${variant}`, className)} {...props}>{children}</div>;
}
