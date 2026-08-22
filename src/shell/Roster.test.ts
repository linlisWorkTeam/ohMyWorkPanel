import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("Roster", () => {
  const src = readFileSync(resolve("src/shell/Roster.tsx"), "utf8");
  it("has contact rows and no ellipsis button", () => {
    expect(src).toContain("wp-m-row");
    expect(src).toContain("useLongPress");
    expect(src).not.toMatch(/className="member-more"/);
    expect(src).not.toContain("⋯");
  });
});
