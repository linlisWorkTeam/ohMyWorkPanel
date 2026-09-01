import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

describe("connecter-remote member form", () => {
  const app = readFileSync("src/App.tsx", "utf8");

  it("shows remote settings instead of CLI executable and model controls", () => {
    expect(app).toContain("const isConnecterRemote =");
    expect(app).toContain("!isConnecterRemote && modelsForAdapter(newMember.adapter).length > 0");
    expect(app).toContain("{isConnecterRemote ? <>");
    expect(app).toMatch(/isConnecterRemote \? <>[\s\S]*connecterBaseUrl[\s\S]*connecterEnv[\s\S]*connecterGroupRef[\s\S]*connecterTargetSubjectId[\s\S]*connecterBearer[\s\S]*: \([\s\S]*executablePath/);
  });

  it("uses a non-autofilled password input for the dedicated bearer", () => {
    expect(app).toMatch(/type="password"[\s\S]{0,160}autoComplete="new-password"[\s\S]{0,160}value=\{newMember\.connecterBearer\}/);
    expect(app).not.toContain("apiKey: newMember.connecterBearer");
  });

  it("builds agent adapter fields through the mutually exclusive payload helper", () => {
    expect(app).toContain("buildAgentAdapterPayload({");
    expect(app).toContain("connecterBearer: newMember.connecterBearer");
  });
});
