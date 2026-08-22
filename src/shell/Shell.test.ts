import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("Shell chrome", () => {
  const src = readFileSync(resolve("src/shell/Shell.tsx"), "utf8");
  const css = readFileSync(resolve("src/shell/tokens.css"), "utf8");

  it("uses mock grid columns rail | left | mid", () => {
    expect(src).toContain('className="wp-rail"');
    expect(src).toContain('className="wp-left"');
    expect(src).toContain('className="wp-mid"');
    expect(src).toContain('className="wp-wave"');
    expect(src).not.toContain("title=\"外观主题\"");
    expect(src).not.toContain(">🎨<");
    expect(css).toMatch(/\.wp-shell\s*\{[^}]*grid-template-columns/);
  });

  it("shows the left column when is-left-open and collapses an empty right column", () => {
    expect(css).toMatch(/\.wp-shell:not\(\.is-left-open\)[^}]*--left:\s*0/);
    expect(css).toMatch(/\.wp-shell:not\(:has\(\.wp-right\s*>\s*\*\)\)[^}]*--right:\s*0/);
  });

  it("zeros overlay columns at ≤1080px and drawers onto wp-left / wp-right", () => {
    const idx = css.indexOf("@media (max-width: 1080px)");
    expect(idx).toBeGreaterThan(-1);
    const chunk = css.slice(idx, idx + 1600);
    expect(chunk).toMatch(/\.wp-shell\s*\{[^}]*--left:\s*0/);
    expect(chunk).toMatch(/\.wp-shell\s*\{[^}]*--right:\s*0/);
    expect(chunk).toMatch(/\.wp-left\s*\{[^}]*position:\s*fixed/);
    expect(chunk).toMatch(/\.wp-right\s*\{[^}]*position:\s*fixed/);
  });

  it("defaults leftMode to rail on max-width 1080px", () => {
    const frame = readFileSync(resolve("src/components/ui/useAppFrame.ts"), "utf8");
    expect(frame).toMatch(/matchMedia\(\s*["']\(max-width:\s*1080px\)["']\s*\)/);
    expect(frame).toMatch(/useState<"open"\s*\|\s*"rail">\(\s*\(\)\s*=>[\s\S]*?["']rail["']/);
  });
});
