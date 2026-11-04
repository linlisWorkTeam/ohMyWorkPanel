import { describe, expect, it } from "vitest";

import { routeChatEvent } from "./eventRouting";

describe("routeChatEvent", () => {
  it("ignores heartbeats and live events", () => {
    expect(routeChatEvent({ kind: "heartbeat" })).toBe("ignore");
    expect(routeChatEvent({ kind: "live_event" })).toBe("ignore");
  });

  it("routes presence snapshot and single presence", () => {
    expect(routeChatEvent({ kind: "presence_snapshot", onlineUserIds: ["a"] })).toBe("presence");
    expect(routeChatEvent({ kind: "presence", userId: "a", online: true })).toBe("presence");
  });

  it("falls back to refresh when presence lacks payload", () => {
    expect(routeChatEvent({ kind: "presence_snapshot" })).toBe("refresh");
    expect(routeChatEvent({ kind: "presence" })).toBe("refresh");
  });

  it("routes orchestration status to full refresh", () => {
    expect(routeChatEvent({ kind: "orchestration_status" })).toBe("orchestration");
  });

  it("routes delta and run status only with ids", () => {
    expect(routeChatEvent({ kind: "message_delta", messageId: "m1" })).toBe("delta");
    expect(routeChatEvent({ kind: "message_delta" })).toBe("refresh");
    expect(routeChatEvent({ kind: "run_status", runId: "r1" })).toBe("run");
    expect(routeChatEvent({ kind: "run_status" })).toBe("refresh");
  });

  it("routes reconnected to resync and unknown kinds to refresh", () => {
    expect(routeChatEvent({ kind: "ws_reconnected" })).toBe("resync");
    expect(routeChatEvent({ kind: "mystery_event" })).toBe("refresh");
  });

  it("keeps precedence: delta before resync before refresh", () => {
    expect(routeChatEvent({ kind: "message_delta", messageId: "m1" })).toBe("delta");
  });
});
