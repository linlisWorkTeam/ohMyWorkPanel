import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("shell tokens match ui-v2.1-shell.html", () => {
  const tokens = readFileSync(resolve("src/shell/tokens.css"), "utf8");
  const mock = readFileSync(resolve("docs/ui-v2.1-shell.html"), "utf8");

  it("copies cyberpunk / atlas / minimal --bg from the mock", () => {
    expect(mock).toMatch(/\[data-theme="cyberpunk"\][\s\S]*--bg:#0a0014/);
    expect(tokens).toMatch(/\[data-theme="cyberpunk"\][\s\S]*--bg:\s*#0a0014/);
    expect(tokens).toMatch(/\[data-theme="atlas"\][\s\S]*--bg:\s*#e7eef5/);
    expect(tokens).toMatch(/\[data-theme="minimal"\][\s\S]*--bg:\s*#f4f5f7/);
    expect(tokens).toMatch(/\[data-theme="minimal"\][\s\S]*--scan:\s*0/);
    expect(tokens).toMatch(/IBM Plex Sans/);
  });
});
