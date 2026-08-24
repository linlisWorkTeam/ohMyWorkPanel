import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("frontend color purity gate", () => {
  it("rejects every color literal outside theme sources", () => {
    const output = execFileSync(process.execPath, [resolve("scripts/check-color-purity.mjs")], {
      cwd: resolve("."),
      encoding: "utf8",
    });
    expect(output).toContain("0 violations");
  });
});
