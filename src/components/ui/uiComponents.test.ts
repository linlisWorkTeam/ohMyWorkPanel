import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const read = (path: string) => readFileSync(resolve(path), "utf8");

describe("shared UI governance", () => {
  it("exports the required primitives", () => {
    const index = read("src/components/ui/index.ts");
    for (const name of ["Modal", "FormField", "ButtonGroup", "UiButton", "Badge", "Toast", "ContextMenu", "PageShell", "ResponsiveGrid"]) {
      expect(index).toContain(name);
    }
  });

  it("defines viewport and container rules for desktop, narrow panes, and touch screens", () => {
    const css = read("src/components/ui/ui.css");
    expect(css).toContain("container-type: inline-size");
    expect(css).toContain("@container (max-width: 560px)");
    expect(css).toContain("@media (max-width: 720px)");
    expect(css).toContain("@media (hover: none), (pointer: coarse)");
    expect(read("src/ExperiencePanel.tsx")).toContain("<PageShell");
    expect(read("src/LogsPanel.tsx")).toContain("<PageShell");
  });

  it("styles primitives with semantic tokens only", () => {
    const css = read("src/components/ui/ui.css");
    expect(css).toContain("var(--lp-bg-elev)");
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(/i);
  });

  it("migrates real Modal, Toast, Badge, and ContextMenu consumers", () => {
    expect(read("src/App.tsx")).toMatch(/import\s*\{[^}]*Modal[^}]*Toast[^}]*\}\s*from\s*["']\.\/components\/ui["']/s);
    expect(read("src/PmPanel.tsx")).toContain("<Badge");
    expect(read("src/components/ContextActionMenu.tsx")).toContain("ContextMenu as ContextActionMenu");
  });
});
