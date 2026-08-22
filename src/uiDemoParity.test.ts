import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("ui-v2.1-shell.html chrome parity", () => {
  const app = readFileSync(resolve("src/App.tsx"), "utf8");
  const tokens = readFileSync(resolve("src/shell/tokens.css"), "utf8");
  const shell = readFileSync(resolve("src/shell/Shell.tsx"), "utf8");
  const chat = readFileSync(resolve("src/shell/ChatTranscript.tsx"), "utf8");
  const roster = readFileSync(resolve("src/shell/Roster.tsx"), "utf8");
  const furniture = readFileSync(resolve("src/components/furniture.tsx"), "utf8");
  const registry = readFileSync(resolve("src/contrib/registry.ts"), "utf8");
  const themes = readFileSync(resolve("src/themes.css"), "utf8");

  it("locks seven mock --bg values and minimal scan 0", () => {
    expect(tokens).toMatch(/\[data-theme="cyberpunk"\][\s\S]*--bg:\s*#0a0014/);
    expect(tokens).toMatch(/\[data-theme="minimal"\][\s\S]*--scan:\s*0/);
  });

  it("Shell has rail left mid and App mounts it", () => {
    expect(shell).toContain("wp-rail");
    expect(app).toContain("<Shell");
    expect(registry).toContain('title: "??"');
  });

  it("bubbles are WeChat corners without fold or ellipsis", () => {
    expect(chat).not.toContain("agent-reply-fold");
    expect(furniture).not.toContain("agent-reply-fold");
    expect(roster).not.toContain("?");
    expect(tokens).toMatch(/\.wp-row\.me\s+\.wp-bub/);
  });

  it("registry exposes four right-tab titles", () => {
    expect(registry).toContain('title: "??"');
    expect(registry).toContain('title: "??"');
    expect(registry).toContain('title: "??"');
    expect(registry).toContain('title: "??"');
  });

  it("App mounts RightDockHost in the Shell right slot", () => {
    expect(app).toMatch(/import\s*\{\s*RightDockHost\s*\}\s*from\s*["']\.\/components\/RightDockHost["']/);
    expect(app).toContain("<RightDockHost");
  });

  it("App mounts Shell and does not inline dsh-rail", () => {
    expect(app).toMatch(/import\s*\{\s*Shell\s*\}\s*from\s*["']\.\/shell\/Shell["']/);
    expect(app).toContain("<Shell");
    expect(app).not.toContain('className="dsh-rail"');
    expect(app).toContain("???? Wave");
    expect(app).not.toContain('title="????"');
    expect(app).not.toContain(">??<");
  });

  it("aliases --lp-bg-app to mock --bg", () => {
    const main = readFileSync(resolve("src/main.tsx"), "utf8");
    expect(main).toMatch(/import\s+["']\.\/shell\/tokens\.css["']/);
    expect(themes).toMatch(/--lp-bg-app:\s*var\(--bg\)/);
    expect(themes).toMatch(/--bg-app-image:\s*none/);
  });

  it("does not paint page chrome over mock --bg or tokens scanline", () => {
    const glows = [...themes.matchAll(/--bg-app-glow:\s*([^;]+);/g)].map((m) => m[1].trim());
    expect(glows.length).toBeGreaterThan(0);
    for (const glow of glows) {
      expect(glow).toBe("none");
    }
    expect(themes).not.toMatch(/background:\s*radial-gradient\(1100px/);
    expect(themes).not.toMatch(/body::before\s*\{[^}]*opacity:\s*0\s*!important/);
    expect(themes).not.toMatch(/body::before\s*\{/);
    expect(tokens).toMatch(/html,\s*body,\s*#root\s*\{[^}]*background:\s*var\(--bg\)/);
    expect(tokens).toMatch(/body::before\s*\{[^}]*opacity:\s*var\(--scan\)/);
  });

  it("theme blocks keep Task 2 aliases on shell --bg", () => {
    expect(themes).not.toMatch(/--s-bg0:\s*#/);
    expect(themes).not.toMatch(/--bg-app:\s*#/);
    expect(themes).not.toMatch(/--lp-bg-app:\s*var\(--s-bg0\)/);
    expect(themes).not.toMatch(/--bg-app:\s*var\(--s-bg0\)/);
    expect(themes).toMatch(/--lp-bg-app:\s*var\(--bg\)/);
    expect(themes).toMatch(/--bg-app:\s*var\(--bg\)/);
    expect(themes).toMatch(/--s-bg0:\s*var\(--bg\)/);
  });

  it("keeps message actions in an overlay menu (no hover layout jump, no always-on copy row)", () => {
    expect(themes).toMatch(/\.ctx-menu\s*\{[^}]*position:\s*fixed/);
    expect(themes).not.toMatch(/\.message-row:hover\s+\.m-actions/);
    expect(furniture).toContain("bubble-stop");
    expect(furniture).not.toContain('className="m-actions"');
  });

  it("does not put unused thumbs on the bubble action row", () => {
    expect(furniture).not.toContain("??");
    expect(furniture).not.toContain("??");
    expect(furniture).not.toContain("voteMessage");
  });

  it("moves member detect/admin/remove into the context menu", () => {
    expect(furniture).toContain('label: detecting ? "???" : "??"');
    expect(furniture).toContain("???");
    expect(furniture).not.toMatch(/<div className="member-actions">/);
  });

  it("has no agent-reply-fold and no member-more ellipsis", () => {
    expect(furniture).not.toContain("agent-reply-fold");
    expect(chat).not.toContain("agent-reply-fold");
    expect(furniture).not.toContain("member-more");
    expect(roster).not.toContain("member-more");
    expect(roster).not.toContain("?");
  });

  it("settings theme cards are mock stage swatches", () => {
    const theme = readFileSync(resolve("src/theme.tsx"), "utf8");
    expect(theme).toContain("wp-stage");
    expect(theme).not.toContain("theme-card-shine");
    expect(theme).not.toContain("theme-card-badge");
  });

  it("auth and error chrome use shell tokens", () => {
    const styles = readFileSync(resolve("src/styles.css"), "utf8");
    expect(styles).toMatch(/\.auth-screen[\s\S]*background:\s*var\(--bg\)/);
    expect(styles).toMatch(/\.auth-card[\s\S]*background:\s*var\(--surf\)/);
  });

  it("does not load ui-demo display fonts or city SVGs", () => {
    expect(themes).not.toMatch(/fonts\.googleapis\.com.*Syne/);
    expect(themes).not.toContain("/themes/cyberpunk.svg");
    expect(themes).not.toContain("/themes/industrial.svg");
  });
});
