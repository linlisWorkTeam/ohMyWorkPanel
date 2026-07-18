// WebSocket-backed stub for @tauri-apps/api/event — used in web builds only.
// Subscribes to server-sent events via a shared WebSocket connection.

type WsListener = (event: { payload: any }) => void;
const listeners = new Map<string, Set<WsListener>>();
let sharedWs: WebSocket | null = null;
let authToken: string | null = null;

/** Store the auth token so the WS connection can pass it as a query param. */
export function _setWebAuthToken(token: string | null) {
  authToken = token;
}

function ensureWs() {
  const url = `ws://${location.host}/ws${authToken ? `?token=${authToken}` : ""}`;
  if (sharedWs && sharedWs.url === url && (sharedWs.readyState === WebSocket.OPEN || sharedWs.readyState === WebSocket.CONNECTING)) {
    return;
  }
  if (sharedWs) {
    try { sharedWs.close(); } catch { /* ignore */ }
  }
  sharedWs = new WebSocket(url);
  sharedWs.onmessage = (e: MessageEvent) => {
    try {
      const data = JSON.parse(e.data as string);
      const kind = data.kind;
      const set = listeners.get(kind);
      if (set) {
        set.forEach((cb) => cb({ payload: data }));
      }
    } catch {
      // non-JSON messages or events don't need dispatching
    }
  };
}

export async function listen<T>(
  event: string,
  callback: (event: { payload: T }) => void,
): Promise<() => void> {
  ensureWs();
  if (!listeners.has(event)) {
    listeners.set(event, new Set());
  }
  listeners.get(event)!.add(callback as WsListener);
  return () => {
    listeners.get(event)?.delete(callback as WsListener);
  };
}

export async function emit(event: string, payload?: unknown): Promise<void> {
  const set = listeners.get(event);
  if (set) {
    set.forEach((cb) => cb({ payload }));
  }
}
