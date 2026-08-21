import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("ui-demo.html chrome parity", () => {
  const app = readFileSync(resolve("src/App.tsx"), "utf8");
  const themes = readFileSync(resolve("src/themes.css"), "utf8");
  const furniture = readFileSync(resolve("src/components/furniture.tsx"), "utf8");
  const registry = readFileSync(resolve("src/contrib/registry.ts"), "utf8");

  it("maps demo --s-* palettes and drops city/industrial background images", () => {
    expect(themes).toMatch(/--s-bg0:\s*#0a0014/);
    expect(themes).toMatch(/--s-bg0:\s*#e7eef5/);
    expect(themes).toMatch(/--s-bg0:\s*#f4f5f7/);
    expect(themes).toMatch(/\[data-theme="cyberpunk"\][\s\S]*--bg-app-image:\s*none/);
    expect(themes).toMatch(/\[data-theme="industrial"\][\s\S]*--bg-app-image:\s*none/);
    expect(themes).toMatch(/\[data-theme="atlas"\][\s\S]*--text-on-sidebar:\s*var\(--s-paper\)/);
    expect(themes).toMatch(/\[data-theme="minimal"\][\s\S]*--scanline-opacity:\s*0/);
  });

  it("uses demo shell DOM: rail, chip header, composer card, four right tabs, persistent goal bar", () => {
    expect(app).toContain("className=\"dsh-rail\"");
    expect(app).toContain("className=\"chip\"");
    expect(app).toContain("className=\"chat-title\"");
    expect(app).toContain("className=\"header-right\"");
    expect(app).toContain("className=\"composer\"");
    expect(app).toContain("className=\"composer-hint\"");
    expect(app).toContain("className=\"send-btn\"");
    expect(app).toContain("rightTabs.map");
    expect(registry).toContain('title: "成员"');
    expect(registry).toContain('title: "队列"');
    expect(registry).toContain('title: "详情"');
    expect(registry).toContain('title: "设置"');
    expect(app).toContain("尚未建立 Wave");
    expect(app).not.toContain("className=\"view-toggle\"");
    expect(app).not.toContain("公告 / 工作目录在「设置」");
    expect(app).not.toContain("title=\"外观主题\"");
    expect(app).not.toContain(">🎨<");
  });

  it("keeps message actions in an overlay menu (no hover layout jump, no always-on copy row)", () => {
    expect(themes).toMatch(/\.ctx-menu\s*\{[^}]*position:\s*fixed/);
    expect(themes).not.toMatch(/\.message-row:hover\s+\.m-actions/);
    expect(furniture).toContain("bubble-stop");
    expect(furniture).not.toContain("className=\"m-actions\"");
  });

  it("does not put unused thumbs on the bubble action row", () => {
    expect(furniture).not.toContain("👍");
    expect(furniture).not.toContain("👎");
    expect(furniture).not.toContain("voteMessage");
  });

  it("moves member detect/admin/remove into the context menu", () => {
    expect(furniture).toContain("label: detecting ? \"检测中\" : \"检测\"");
    expect(furniture).toContain("设管理");
    expect(furniture).not.toMatch(/<div className="member-actions">/);
  });
});
