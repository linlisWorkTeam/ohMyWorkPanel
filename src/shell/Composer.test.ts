import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("Composer chrome", () => {
  const src = readFileSync(resolve("src/shell/Composer.tsx"), "utf8");
  const app = readFileSync(resolve("src/App.tsx"), "utf8");
  const tokens = readFileSync(resolve("src/shell/tokens.css"), "utf8");

  it("has quote bar and composer card classes", () => {
    expect(src).toContain("wp-quote-bar");
    expect(src).toContain("wp-composer");
    expect(src).toContain("onClearQuote");
  });

  it("keeps @ / slash menus in a padding-free positioning wrap", () => {
    expect(app).toContain('className="wp-composer-anchor"');
    expect(app).not.toContain('className="composer-wrap"');
    expect(app).toContain("mention-menu");
    expect(app).toContain("slash-menu");
    const anchor = tokens.match(/\.wp-composer-anchor\s*\{[^}]*\}/);
    expect(anchor?.[0]).toMatch(/position:\s*relative/);
    expect(anchor?.[0]).toMatch(/padding:\s*0/);
    expect(anchor?.[0]).toMatch(/background:\s*none/);
    expect(anchor?.[0]).toMatch(/border:\s*0/);
    expect(anchor?.[0]).not.toMatch(/padding:\s*1/);
  });
});
