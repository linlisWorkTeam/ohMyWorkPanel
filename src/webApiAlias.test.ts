import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

describe("web api alias", () => {
  it("aliases every relative import of src/api.ts, including src/components/furniture.tsx", () => {
    const vite = readFileSync("vite.config.web.ts", "utf8");
    const furniture = readFileSync("src/components/furniture.tsx", "utf8");
    expect(furniture).toMatch(/from ["']\.\.\/api["']/);
    // P1 extracted LazyChannelPart into components/; `/^\.\/api$/` only rewrites App.tsx
    // and clicking 思考过程 would call Tauri invoke in the browser (empty #root).
    expect(vite).not.toContain("find: /^\\.\\/api$/");
    expect(vite).toContain("^(?:\\.\\.\\/)+api$");
    expect(vite).toContain("^\\.\\/api$");
  });
});
