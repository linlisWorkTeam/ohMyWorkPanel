// WebSocket-backed stub for @tauri-apps/api/event — used in web builds only.
// Subscribes to server-sent events via a shared WebSocket connection.

type WsListener = (event: { payload: any }) => void;
const listeners = new Map<string, Set<WsListener>>();
let sharedWs: WebSocket | null = null;
let authToken: string | null = null;
const TOKEN_KEY = "linlis_auth_token";

/** Store the auth token so the WS connection can pass it as a query param. */
export function _setWebAuthToken(token: string | null) {
  authToken = token;
}

function resolveToken(): string | null {
  if (authToken) return authToken;
  try {
    return localStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

/** Normalize backend payloads to the camelCase ChatEvent shape App.tsx expects. */
function normalizePayload(data: Record<string, unknown>) {
  return {
    kind: data.kind,
    groupId: data.groupId ?? data.group_id ?? null,
    runId: data.runId ?? data.run_id ?? null,
    messageId: data.messageId ?? data.message_id ?? null,
    delta: data.delta ?? null,
    status: data.status ?? null,
    error: data.error ?? null,
    channel: data.channel ?? null,
    replace: data.replace ?? null,
  };
}

function dispatch(eventName: string, payload: unknown) {
  const set = listeners.get(eventName);
  if (!set) return;
  set.forEach((cb) => cb({ payload }));
}

let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let reconnectAttempts = 0;

function scheduleReconnect() {
  if (reconnectTimer) return;
  if (listeners.size === 0) return;
  reconnectAttempts += 1;
  const delay = Math.min(30000, 1000 * 2 ** (reconnectAttempts - 1));
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    ensureWs();
  }, delay);
}

function ensureWs() {
  const token = resolveToken();
  const url = `ws://${location.host}/ws${token ? `?token=${encodeURIComponent(token)}` : ""}`;
  if (sharedWs && sharedWs.url === url && (sharedWs.readyState === WebSocket.OPEN || sharedWs.readyState === WebSocket.CONNECTING)) {
    return;
  }
  if (sharedWs) {
    try { sharedWs.close(); } catch { /* ignore */ }
  }
  sharedWs = new WebSocket(url);
  sharedWs.onopen = () => { reconnectAttempts = 0; };
  sharedWs.onmessage = (e: MessageEvent) => {
    try {
      const data = JSON.parse(e.data as string) as Record<string, unknown>;
      const payload = normalizePayload(data);
      // Match Tauri desktop: App listens on "chat-event"
      dispatch("chat-event", payload);
      // Also allow kind-specific listeners if any
      if (typeof payload.kind === "string") {
        dispatch(payload.kind, payload);
      }
    } catch {
      // non-JSON messages or events don't need dispatching
    }
  };
  sharedWs.onclose = () => scheduleReconnect();
  sharedWs.onerror = () => { try { sharedWs?.close(); } catch { /* ignore */ } };
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
