import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("v2.1.1 version lock", () => {
  it("package and cargo are 2.1.1", () => {
    const pkg = JSON.parse(readFileSync(resolve("package.json"), "utf8"));
    const cargo = readFileSync(resolve("src-tauri/Cargo.toml"), "utf8");
    expect(pkg.version).toBe("2.1.1");
    expect(cargo).toMatch(/^version = "2.1.1"/m);
  });
});
