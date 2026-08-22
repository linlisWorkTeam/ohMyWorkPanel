import type { ContribMotion, ContribSlot, UiContribution } from "./types";

export const BASE_RIGHT_TABS: UiContribution[] = [
  { id: "core.members", title: "成员", slot: "right-tab", origin: "base", order: 10 },
  { id: "core.queue", title: "队列", slot: "right-tab", origin: "base", order: 20 },
  { id: "core.details", title: "详情", slot: "right-tab", origin: "base", order: 30 },
  { id: "core.settings", title: "设置", slot: "right-tab", origin: "base", order: 40 },
];

export const BASE_COMPOSER_TOOLS: UiContribution[] = [
  { id: "core.mention", title: "@", slot: "composer-tool", origin: "base", order: 10 },
  { id: "core.slash", title: "/", slot: "composer-tool", origin: "base", order: 20 },
  { id: "core.ocr", title: "🖼", slot: "composer-tool", origin: "base", order: 30 },
];

export const BASE_MESSAGE_ACTIONS: UiContribution[] = [
  { id: "core.msg.copy", title: "复制", slot: "message-action", origin: "base", order: 10 },
  { id: "core.msg.quote", title: "引用", slot: "message-action", origin: "base", order: 20 },
  { id: "core.msg.speak", title: "朗读", slot: "message-action", origin: "base", order: 30 },
  { id: "core.msg.retry", title: "重试", slot: "message-action", origin: "base", order: 40 },
];

export function coreRegistry(): UiContribution[] {
  return [...BASE_RIGHT_TABS, ...BASE_COMPOSER_TOOLS, ...BASE_MESSAGE_ACTIONS];
}

export function listContributions(
  extras: UiContribution[],
  slot?: ContribSlot,
): UiContribution[] {
  const merged = [...coreRegistry(), ...extras];
  const filtered = slot ? merged.filter((item) => item.slot === slot) : merged;
  return filtered.slice().sort((a, b) => (a.order ?? 100) - (b.order ?? 100) || a.id.localeCompare(b.id));
}

export function effectiveMotion(
  contribution: UiContribution,
  themeId: string,
  reducedMotion: boolean,
): ContribMotion {
  if (reducedMotion || themeId === "minimal") return "none";
  return contribution.motion ?? "none";
}
