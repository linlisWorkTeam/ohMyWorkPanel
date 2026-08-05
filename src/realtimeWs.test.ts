import { describe, expect, it } from "vitest";
import {
  isIgnorableWsKind,
  reconnectDelayMs,
  shouldResyncAfterWsEvent,
} from "./realtimeWs";

describe("realtimeWs", () => {
  it("ignores heartbeat frames in chat UI handlers", () => {
    expect(isIgnorableWsKind("heartbeat")).toBe(true);
    expect(isIgnorableWsKind("live_event")).toBe(true);
    expect(isIgnorableWsKind("message_delta")).toBe(false);
  });

  it("asks for resync after reconnect", () => {
    expect(shouldResyncAfterWsEvent("ws_reconnected")).toBe(true);
    expect(shouldResyncAfterWsEvent("heartbeat")).toBe(false);
    expect(shouldResyncAfterWsEvent("run_status")).toBe(false);
  });

  it("backs off reconnect delays", () => {
    expect(reconnectDelayMs(1)).toBe(1000);
    expect(reconnectDelayMs(2)).toBe(2000);
    expect(reconnectDelayMs(10)).toBe(30000);
  });
});
