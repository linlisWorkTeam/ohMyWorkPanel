import { describe, expect, it } from "vitest";
import {
  CONNECTER_REMOTE_ADAPTER,
  FALLBACK_CLI_ADAPTERS,
  buildAgentAdapterPayload,
  mergeCliAdapters,
} from "./adaptersCatalog";

describe("mergeCliAdapters", () => {
  it("falls back when remote is empty", () => {
    expect(mergeCliAdapters([])).toEqual(FALLBACK_CLI_ADAPTERS);
    expect(mergeCliAdapters(undefined).some((a) => a.id === "mock")).toBe(true);
    expect(mergeCliAdapters(undefined)).toContainEqual(CONNECTER_REMOTE_ADAPTER);
  });

  it("merges the built-in remote provider into a dynamic plugin catalog", () => {
    const rows = [{ id: "acme-cli", displayName: "Acme" }];
    expect(mergeCliAdapters(rows)).toEqual([...rows, CONNECTER_REMOTE_ADAPTER]);
  });

  it("does not duplicate the remote provider supplied by the server", () => {
    const rows = [CONNECTER_REMOTE_ADAPTER, { id: "acme-cli", displayName: "Acme" }];
    expect(mergeCliAdapters(rows)).toEqual(rows);
  });
});

describe("buildAgentAdapterPayload", () => {
  const draft = {
    adapter: "connecter-remote",
    executablePath: "codex",
    model: "gpt-5",
    connecterBaseUrl: " http://connecter.test:9080/ ",
    connecterEnv: " canary ",
    connecterGroupRef: " wp:ecs-canary:seed-group-ohmyworkpanel ",
    connecterTargetSubjectId: " codex-windows11 ",
    connecterBearer: "secret-token",
  };

  it("submits only provider fields for connecter-remote", () => {
    expect(buildAgentAdapterPayload(draft)).toEqual({
      adapter: "connecter-remote",
      connecterBaseUrl: "http://connecter.test:9080/",
      connecterEnv: "canary",
      connecterGroupRef: "wp:ecs-canary:seed-group-ohmyworkpanel",
      connecterTargetSubjectId: "codex-windows11",
      connecterBearer: "secret-token",
    });
    expect(buildAgentAdapterPayload(draft)).not.toHaveProperty("executablePath");
    expect(buildAgentAdapterPayload(draft)).not.toHaveProperty("model");
    expect(buildAgentAdapterPayload(draft)).not.toHaveProperty("apiKey");
  });

  it("never sends provider fields for a normal CLI adapter", () => {
    const payload = buildAgentAdapterPayload({ ...draft, adapter: "codex" });
    expect(payload).toEqual({ adapter: "codex", executablePath: "codex", model: "gpt-5" });
    expect(payload).not.toHaveProperty("connecterBearer");
    expect(payload).not.toHaveProperty("connecterBaseUrl");
  });
});
