import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("theme chrome imports stay alive in App.tsx", () => {
  const app = readFileSync(resolve("src/App.tsx"), "utf8");

  it("value-imports Brand / ThemeSwitcher / HeaderThemePop from ./theme", () => {
    expect(app).toMatch(
      /import\s*\{[^}]*\bBrand\b[^}]*\bThemeSwitcher\b[^}]*\bHeaderThemePop\b[^}]*\}\s*from\s*["']\.\/theme["']/,
    );
  });

  it("uses those names as values, not only JSX tags (JSX-only imports can be stripped → empty #root)", () => {
    const withoutImport = app.replace(/import\s*\{[^}]*\}\s*from\s*["']\.\/theme["'];?/, "");
    const withoutJsxTags = withoutImport
      .replace(/<Brand\b[^>]*\/?>/g, "")
      .replace(/<ThemeSwitcher\b[^>]*\/?>/g, "")
      .replace(/<HeaderThemePop\b[^>]*\/?>/g, "");
    expect(withoutJsxTags).toMatch(/\bBrand\s*\(|=\s*Brand\b/);
    expect(withoutJsxTags).toMatch(/\bThemeSwitcher\s*\(|=\s*ThemeSwitcher\b/);
    expect(withoutJsxTags).toMatch(/\bHeaderThemePop\s*\(|=\s*HeaderThemePop\b/);
  });
});
