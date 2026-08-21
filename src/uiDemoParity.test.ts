import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("ui-demo.html chrome parity", () => {
  const app = readFileSync(resolve("src/App.tsx"), "utf8");
  const themes = readFileSync(resolve("src/themes.css"), "utf8");

  it("maps demo --s-* palettes and drops city/industrial background images", () => {
    expect(themes).toMatch(/--s-bg0:\s*#0a0014/);
    expect(themes).toMatch(/--s-bg0:\s*#e7eef5/);
    expect(themes).toMatch(/\[data-theme="cyberpunk"\][\s\S]*--bg-app-image:\s*none/);
    expect(themes).toMatch(/\[data-theme="industrial"\][\s\S]*--bg-app-image:\s*none/);
    expect(themes).toMatch(/\[data-theme="atlas"\][\s\S]*--text-on-sidebar:\s*var\(--s-paper\)/);
  });

  it("uses demo shell DOM: rail, chip header, composer card, 3 right tabs, persistent goal bar", () => {
    expect(app).toContain("className=\"dsh-rail\"");
    expect(app).toContain("className=\"chip\"");
    expect(app).toContain("className=\"chat-title\"");
    expect(app).toContain("className=\"header-right\"");
    expect(app).toContain("className=\"composer\"");
    expect(app).toContain("className=\"composer-hint\"");
    expect(app).toContain("className=\"send-btn\"");
    expect(app).toContain(">成员</button>");
    expect(app).toContain(">队列</button>");
    expect(app).toContain(">详情</button>");
    expect(app).toContain("尚未建立 Wave");
    expect(app).not.toContain("className=\"view-toggle\"");
    expect(app).not.toContain("公告 / 工作目录在「设置」");
  });

  it("keeps message action row always visible (no hover layout jump)", () => {
    expect(themes).toMatch(/\.m-actions\s*\{[^}]*display:\s*flex/);
    expect(themes).not.toMatch(/\.message-row:hover\s+\.m-actions/);
  });

  it("does not put unused thumbs on the bubble action row", () => {
    const furniture = readFileSync(resolve("src/components/furniture.tsx"), "utf8");
    expect(furniture).not.toContain("👍");
    expect(furniture).not.toContain("👎");
    expect(furniture).not.toContain("voteMessage");
  });
});
