import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const THEME_IDS = ["atlas", "cyberpunk", "industrial", "forge", "moss", "noir", "minimal"];
const REQUIRED_PALETTE_TOKENS = [
  "--acc", "--acc2", "--bg", "--breath", "--critical", "--dim", "--elev",
  "--grain", "--ink", "--line", "--ok", "--on-accent", "--radius-mode", "--scan",
  "--shadow-color", "--surf", "--user", "--warning",
].sort();
const REQUIRED_SEMANTIC_TOKENS = [
  "--lp-accent", "--lp-accent-soft", "--lp-accent-strong", "--lp-bg-active", "--lp-bg-app",
  "--lp-bg-elev", "--lp-bg-elevated", "--lp-bg-hover", "--lp-bg-overlay", "--lp-bg-panel",
  "--lp-bg-sidebar", "--lp-bg-surface", "--lp-border", "--lp-border-l1", "--lp-border-l2",
  "--lp-border-strong", "--lp-bubble-ai", "--lp-bubble-user", "--lp-composer", "--lp-disabled-bg",
  "--lp-disabled-text", "--lp-error", "--lp-radius-lg", "--lp-radius-md", "--lp-radius-sm",
  "--lp-rail", "--lp-ring", "--lp-scroll", "--lp-shadow", "--lp-shadow-color", "--lp-success", "--lp-text",
  "--lp-text-dim", "--lp-text-faint", "--lp-text-invert", "--lp-text-muted", "--lp-text-on-sidebar",
  "--lp-text-primary", "--lp-text-secondary", "--lp-text-tertiary", "--lp-warn",
].sort();

function declaredTokens(body: string, prefix = "--") {
  return [...body.matchAll(new RegExp(`(${prefix}[a-z0-9-]+)\\s*:`, "gi"))].map((match) => match[1]).sort();
}

describe("theme token governance", () => {
  const palettes = readFileSync(resolve("src/shell/tokens.css"), "utf8");
  const themes = readFileSync(resolve("src/themes.css"), "utf8");

  it("defines the same complete palette contract for all seven themes", () => {
    const blocks = new Map<string, string>();
    for (const match of palettes.matchAll(/\[data-theme="([^"]+)"\]\s*\{([^}]+)\}/g)) {
      blocks.set(match[1], match[2]);
    }
    expect([...blocks.keys()].sort()).toEqual([...THEME_IDS].sort());
    for (const id of THEME_IDS) {
      const tokens = declaredTokens(blocks.get(id) ?? "");
      expect(tokens, `${id} palette tokens`).toEqual(REQUIRED_PALETTE_TOKENS);
    }
  });

  it("publishes one stable semantic component contract", () => {
    const block = themes.match(/THEME_SEMANTIC_TOKENS_START([\s\S]*?)THEME_SEMANTIC_TOKENS_END/)?.[1] ?? "";
    expect(block).toContain('[data-theme="minimal"]');
    expect(declaredTokens(block, "--lp-")).toEqual(REQUIRED_SEMANTIC_TOKENS);
  });

  it("keeps legacy names as aliases instead of literals", () => {
    const block = themes.match(/THEME_SEMANTIC_TOKENS_START([\s\S]*?)THEME_SEMANTIC_TOKENS_END/)?.[1] ?? "";
    for (const legacy of ["bg-app", "bg-surface", "bg-modal", "text", "border", "accent", "danger", "error", "ring"]) {
      expect(block).toMatch(new RegExp(`--${legacy}:\\s*var\\(--lp-`));
    }
  });
});
