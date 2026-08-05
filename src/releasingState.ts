/** Publish / reconnect wait window (P2). */
export const RELEASING_WINDOW_MS = 60_000;
export const RELEASING_HEALTH_POLL_MS = 5_000;

export type WsLinkState = "connected" | "releasing" | "timeout";

export function nextLinkStateOnClose(now: number, closedAt: number): WsLinkState {
  return now - closedAt >= RELEASING_WINDOW_MS ? "timeout" : "releasing";
}

export function releasingBannerText(state: WsLinkState, elapsedMs: number): string | null {
  if (state === "connected") return null;
  if (state === "timeout") {
    return "重连超时：发布/网络中断超过 60s，请手动刷新页面。";
  }
  const left = Math.max(0, Math.ceil((RELEASING_WINDOW_MS - elapsedMs) / 1000));
  return `发布中/重连中… 剩余约 ${left}s（探活 /api/health）`;
}

export function shouldKeepReleasing(healthOk: boolean, wsOpen: boolean, elapsedMs: number): boolean {
  if (elapsedMs >= RELEASING_WINDOW_MS) return false;
  // Health up but WS not yet — keep waiting inside the window.
  if (healthOk && !wsOpen) return true;
  if (!wsOpen) return true;
  return false;
}
