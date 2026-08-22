import { describe, expect, it } from "vitest";
import { FALLBACK_CLI_ADAPTERS, mergeCliAdapters } from "./adaptersCatalog";

describe("mergeCliAdapters", () => {
  it("falls back when remote is empty", () => {
    expect(mergeCliAdapters([])).toEqual(FALLBACK_CLI_ADAPTERS);
    expect(mergeCliAdapters(undefined).some((a) => a.id === "mock")).toBe(true);
  });

  it("uses remote catalog including plugin ids", () => {
    const rows = [{ id: "acme-cli", displayName: "Acme" }];
    expect(mergeCliAdapters(rows)).toEqual(rows);
  });
});
