import type { WsLinkState } from "./releasingState";

/** Client → server / server → client keep-alive kind. */
export const WS_HEARTBEAT_KIND = "heartbeat";
/** Fired once after the shared socket opens (including reconnects). */
export const WS_RECONNECTED_KIND = "ws_reconnected";

export const WS_CLIENT_HEARTBEAT_MS = 20_000;
export const WS_RECONNECT_MAX_MS = 30_000;

type LinkListener = (state: WsLinkState, elapsedMs: number) => void;
const linkListeners = new Set<LinkListener>();

/** Web stub publishes link state; desktop no-ops. */
export function publishWsLinkState(state: WsLinkState, elapsedMs = 0) {
  linkListeners.forEach((cb) => cb(state, elapsedMs));
}

export function subscribeWsLinkState(cb: LinkListener): () => void {
  linkListeners.add(cb);
  return () => { linkListeners.delete(cb); };
}

export function isIgnorableWsKind(kind: string | null | undefined): boolean {
  // live_event: A2A control-plane (A1 — not applied to chat message list)
  // run_heartbeat is handled separately (progress) — not ignorable here
  return kind === WS_HEARTBEAT_KIND || kind === "live_event";
}

export function shouldResyncAfterWsEvent(kind: string | null | undefined): boolean {
  return kind === WS_RECONNECTED_KIND;
}

export function reconnectDelayMs(attempt: number): number {
  const n = Math.max(1, attempt);
  return Math.min(WS_RECONNECT_MAX_MS, 1000 * 2 ** (n - 1));
}

export function heartbeatPayload(ts = Date.now()): string {
  return JSON.stringify({ kind: WS_HEARTBEAT_KIND, ts });
}
