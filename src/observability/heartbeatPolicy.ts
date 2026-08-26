import type { RuntimeSettings } from "../types";

export function effectiveHeartbeatSeconds(opts: {
  focused: boolean;
  settings: Pick<
    RuntimeSettings,
    "heartbeatAuto" | "heartbeatFocusSeconds" | "heartbeatBackgroundSeconds"
  >;
  memoryPressure?: boolean;
}): number {
  const focus = Math.max(1, opts.settings.heartbeatFocusSeconds || 1);
  const background = Math.max(1, opts.settings.heartbeatBackgroundSeconds || 5);
  let secs = opts.focused ? focus : background;
  if (opts.settings.heartbeatAuto !== false && opts.memoryPressure) {
    secs = Math.max(secs, background);
  }
  return secs;
}

export function formatHeartbeatLabel(opts: {
  focused: boolean;
  settings: Pick<
    RuntimeSettings,
    "heartbeatAuto" | "heartbeatFocusSeconds" | "heartbeatBackgroundSeconds"
  >;
  memoryPressure?: boolean;
}): string {
  const focus = Math.max(1, opts.settings.heartbeatFocusSeconds || 1);
  const background = Math.max(1, opts.settings.heartbeatBackgroundSeconds || 5);
  const auto = opts.settings.heartbeatAuto !== false;
  const effective = effectiveHeartbeatSeconds(opts);
  const mode = auto ? "Auto" : "手动";
  return `心跳：聚焦 ${focus}s · 后台 ${background}s（${mode}）· 当前 ${effective}s`;
}

export function detectMemoryPressure(deviceMemoryGb?: number): boolean {
  if (deviceMemoryGb == null || !Number.isFinite(deviceMemoryGb)) return false;
  return deviceMemoryGb <= 4;
}
