export type SendKeyMode = "enter" | "ctrlEnter";

const STORAGE_KEY = "ohmyworkpanel_send_key_mode";

export function loadSendKeyMode(): SendKeyMode {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v === "ctrlEnter" || v === "enter") return v;
  } catch {
    /* ignore */
  }
  return "enter";
}

export function saveSendKeyMode(mode: SendKeyMode): void {
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    /* ignore */
  }
}

/** Whether this keydown should send (not newline / mention). */
export function shouldSendOnKey(
  mode: SendKeyMode,
  key: string,
  shiftKey: boolean,
  ctrlKey: boolean,
  metaKey: boolean,
): boolean {
  if (key !== "Enter") return false;
  if (mode === "enter") return !shiftKey;
  return ctrlKey || metaKey;
}

export function sendKeyHint(mode: SendKeyMode): string {
  return mode === "enter"
    ? "Enter 发送 · Shift+Enter 换行"
    : "Ctrl+Enter 发送 · Enter 换行";
}
