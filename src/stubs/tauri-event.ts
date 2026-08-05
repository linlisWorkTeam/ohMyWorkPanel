// WebSocket-backed stub for @tauri-apps/api/event — used in web builds only.
// Subscribes to server-sent events via a shared WebSocket connection.

import {
  heartbeatPayload,
  isIgnorableWsKind,
  publishWsLinkState,
  reconnectDelayMs,
  WS_CLIENT_HEARTBEAT_MS,
  WS_RECONNECTED_KIND,
} from "../realtimeWs";
import {
  nextLinkStateOnClose,
  RELEASING_HEALTH_POLL_MS,
  RELEASING_WINDOW_MS,
  type WsLinkState,
} from "../releasingState";

type WsListener = (event: { payload: any }) => void;
const listeners = new Map<string, Set<WsListener>>();
let sharedWs: WebSocket | null = null;
let authToken: string | null = null;
let heartbeatTimer: ReturnType<typeof setInterval> | null = null;
let visibilityBound = false;
let linkState: WsLinkState = "connected";
let closedAt = 0;
let releasingTimer: ReturnType<typeof setInterval> | null = null;
let healthTimer: ReturnType<typeof setInterval> | null = null;
const TOKEN_KEY = "linlis_auth_token";

function publishLink(state: WsLinkState) {
  linkState = state;
  const elapsed = closedAt ? Date.now() - closedAt : 0;
  publishWsLinkState(state, elapsed);
}

function stopReleasingWatch() {
  if (releasingTimer) { clearInterval(releasingTimer); releasingTimer = null; }
  if (healthTimer) { clearInterval(healthTimer); healthTimer = null; }
}

function startReleasingWatch() {
  stopReleasingWatch();
  closedAt = Date.now();
  publishLink("releasing");
  healthTimer = setInterval(() => {
    void fetch("/api/health", { cache: "no-store" }).catch(() => undefined);
  }, RELEASING_HEALTH_POLL_MS);
  releasingTimer = setInterval(() => {
    const state = nextLinkStateOnClose(Date.now(), closedAt);
    publishLink(state);
    if (state === "timeout") stopReleasingWatch();
  }, 1000);
  // Cap watch at window
  setTimeout(() => {
    if (linkState === "releasing") {
      publishLink("timeout");
      stopReleasingWatch();
    }
  }, RELEASING_WINDOW_MS + 50);
}

function wsUrl(token: string | null): string {
  // Must match page protocol: https pages require wss (ws:// is blocked as mixed content).
  const protocol = location.protocol === "https:" ? "wss:" : "ws:";
  const q = token ? `?token=${encodeURIComponent(token)}` : "";
  return `${protocol}//${location.host}/ws${q}`;
}

/** Store the auth token so the WS connection can pass it as a query param. */
export function _setWebAuthToken(token: string | null) {
  authToken = token;
  // Reconnect with the new token (login/logout), otherwise a pre-auth socket stays dead.
  if (sharedWs) {
    try { sharedWs.close(); } catch { /* ignore */ }
    sharedWs = null;
  }
  stopHeartbeat();
  if (token && listeners.size > 0) ensureWs();
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
    phase: data.phase ?? null,
    elapsedMs: data.elapsedMs ?? data.elapsed_ms ?? null,
    totalMs: data.totalMs ?? data.total_ms ?? null,
    seq: data.seq ?? null,
    deltaCount: data.deltaCount ?? data.delta_count ?? null,
    rssMib: data.rssMib ?? data.rss_mib ?? null,
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
  const delay = reconnectDelayMs(reconnectAttempts);
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    ensureWs();
  }, delay);
}

function stopHeartbeat() {
  if (heartbeatTimer) {
    clearInterval(heartbeatTimer);
    heartbeatTimer = null;
  }
}

function startHeartbeat(ws: WebSocket) {
  stopHeartbeat();
  heartbeatTimer = setInterval(() => {
    if (ws.readyState !== WebSocket.OPEN) return;
    try {
      ws.send(heartbeatPayload());
    } catch {
      try { ws.close(); } catch { /* ignore */ }
    }
  }, WS_CLIENT_HEARTBEAT_MS);
}

function bindVisibility() {
  if (visibilityBound || typeof document === "undefined") return;
  visibilityBound = true;
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState !== "visible") return;
    if (listeners.size === 0) return;
    ensureWs();
    // Tab focused again: ask UI to pull latest in case we missed frames while backgrounded.
    dispatch("chat-event", { kind: WS_RECONNECTED_KIND, groupId: null });
  });
}

function ensureWs() {
  const token = resolveToken();
  const url = wsUrl(token);
  if (sharedWs && sharedWs.url === url && (sharedWs.readyState === WebSocket.OPEN || sharedWs.readyState === WebSocket.CONNECTING)) {
    return;
  }
  if (sharedWs) {
    try { sharedWs.close(); } catch { /* ignore */ }
  }
  stopHeartbeat();
  sharedWs = new WebSocket(url);
  const ws = sharedWs;
  ws.onopen = () => {
    reconnectAttempts = 0;
    stopReleasingWatch();
    publishLink("connected");
    startHeartbeat(ws);
    dispatch("chat-event", { kind: WS_RECONNECTED_KIND, groupId: null });
  };
  ws.onmessage = (e: MessageEvent) => {
    try {
      const data = JSON.parse(e.data as string) as Record<string, unknown>;
      const payload = normalizePayload(data);
      if (isIgnorableWsKind(typeof payload.kind === "string" ? payload.kind : null)) {
        return;
      }
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
  ws.onclose = () => {
    stopHeartbeat();
    startReleasingWatch();
    scheduleReconnect();
  };
  ws.onerror = () => { try { ws.close(); } catch { /* ignore */ } };
  bindVisibility();
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
