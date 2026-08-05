import { describe, expect, it } from "vitest";
import { parseInviteTokenFromPath } from "./InviteLanding";

describe("parseInviteTokenFromPath", () => {
  it("parses /invite/{token}", () => {
    expect(parseInviteTokenFromPath("/invite/abc-123")).toBe("abc-123");
    expect(parseInviteTokenFromPath("/invite/abc-123/")).toBe("abc-123");
  });

  it("rejects other paths", () => {
    expect(parseInviteTokenFromPath("/")).toBeNull();
    expect(parseInviteTokenFromPath("/invite")).toBeNull();
    expect(parseInviteTokenFromPath("/invite/a/b")).toBeNull();
  });
});
