import type { ExtensionStatus } from "./types";

export function panelliveStatus(extensions: ExtensionStatus[] | null | undefined): ExtensionStatus | null {
  return extensions?.find((e) => e.id === "panellive") ?? null;
}

export function liveTabEnabled(ext: ExtensionStatus | null | undefined): boolean {
  return Boolean(ext?.enabled && ext.healthy);
}

export function liveEntryUrl(ext: ExtensionStatus | null | undefined): string | null {
  if (!ext?.enabled) return null;
  const tab = ext.tabs?.find((t) => t.id === "live" || t.route === "tab://live");
  if (!tab?.entry) return null;
  const entry = tab.entry.startsWith("/") ? tab.entry : `/${tab.entry}`;
  const base = (ext.baseUrl ?? "").replace(/\/$/, "");
  return base ? `${base}${entry}` : null;
}
