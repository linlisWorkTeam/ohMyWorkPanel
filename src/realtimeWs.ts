/** Client → server / server → client keep-alive kind. */
export const WS_HEARTBEAT_KIND = "heartbeat";
/** Fired once after the shared socket opens (including reconnects). */
export const WS_RECONNECTED_KIND = "ws_reconnected";

export const WS_CLIENT_HEARTBEAT_MS = 20_000;
export const WS_RECONNECT_MAX_MS = 30_000;

export function isIgnorableWsKind(kind: string | null | undefined): boolean {
  return kind === WS_HEARTBEAT_KIND;
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
