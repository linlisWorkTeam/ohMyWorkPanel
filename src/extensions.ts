import type { ExtensionStatus, ExtensionTab } from "./types";

export type ExtensionTabView = {
  ext: ExtensionStatus;
  tab: ExtensionTab;
  viewKey: string;
};

export function panelliveStatus(extensions: ExtensionStatus[] | null | undefined): ExtensionStatus | null {
  return extensions?.find((e) => e.id === "panellive") ?? null;
}

export function liveTabEnabled(ext: ExtensionStatus | null | undefined): boolean {
  return Boolean(ext?.enabled && ext.healthy);
}

export function liveEntryUrl(ext: ExtensionStatus | null | undefined): string | null {
  if (!ext?.enabled) return null;
  const tab = ext.tabs?.find((t) => t.id === "live" || t.route === "tab://live");
  if (!tab) return null;
  return extensionEntryUrl(ext, tab);
}

export function extensionEntryUrl(
  ext: ExtensionStatus | null | undefined,
  tab: ExtensionTab | null | undefined,
): string | null {
  if (!ext?.enabled || !tab?.entry) return null;
  const entry = tab.entry.startsWith("/") ? tab.entry : `/${tab.entry}`;
  const base = (ext.baseUrl ?? "").replace(/\/$/, "");
  return base ? `${base}${entry}` : null;
}

export function extMainViewKey(extId: string, tabId: string): string {
  return `ext:${extId}:${tabId}`;
}

export function parseExtMainView(view: string): { extId: string; tabId: string } | null {
  if (!view.startsWith("ext:")) return null;
  const rest = view.slice(4);
  const i = rest.indexOf(":");
  if (i <= 0) return null;
  return { extId: rest.slice(0, i), tabId: rest.slice(i + 1) };
}

/** peerOf empty → all groups; otherwise require chat (compat) or matching kind. */
export function tabPeerAllowed(tab: ExtensionTab, groupKind: string): boolean {
  const peers = tab.peerOf ?? [];
  if (peers.length === 0) return true;
  if (peers.includes("chat")) return true;
  return peers.includes(groupKind);
}

/** Tabs to render in the header (enabled extensions; gray offline via healthy). */
export function collectExtensionTabViews(
  extensions: ExtensionStatus[] | null | undefined,
  groupKind: string,
): ExtensionTabView[] {
  const out: ExtensionTabView[] = [];
  for (const ext of extensions ?? []) {
    for (const tab of ext.tabs ?? []) {
      if (!tabPeerAllowed(tab, groupKind)) continue;
      out.push({ ext, tab, viewKey: extMainViewKey(ext.id, tab.id) });
    }
  }
  return out;
}
