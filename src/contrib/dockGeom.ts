export const DOCK_STORAGE_KEY = "lp.rightDock.v1";
export const DOCK_MIN_WIDTH = 220;
export const DOCK_DEFAULT_WIDTH = 260;

export type DockGeom = {
  dockedId: string | null;
  width: number;
};

export function parseDockGeom(raw: string | null | undefined): DockGeom {
  const fallback: DockGeom = { dockedId: null, width: DOCK_DEFAULT_WIDTH };
  if (!raw) return fallback;
  try {
    const parsed = JSON.parse(raw) as Partial<DockGeom>;
    const width = Number(parsed.width);
    return {
      dockedId: typeof parsed.dockedId === "string" && parsed.dockedId ? parsed.dockedId : null,
      width: Number.isFinite(width) ? Math.max(DOCK_MIN_WIDTH, Math.round(width)) : DOCK_DEFAULT_WIDTH,
    };
  } catch {
    return fallback;
  }
}

export function readDockGeom(): DockGeom {
  try {
    return parseDockGeom(sessionStorage.getItem(DOCK_STORAGE_KEY));
  } catch {
    return parseDockGeom(null);
  }
}

export function writeDockGeom(geom: DockGeom): void {
  try {
    sessionStorage.setItem(DOCK_STORAGE_KEY, JSON.stringify(geom));
  } catch {
    /* ignore quota / private mode */
  }
}

/** Narrow right rail cannot host a second column. */
export function shouldCollapseDock(panelWidth: number): boolean {
  return panelWidth < DOCK_MIN_WIDTH * 2 + 8;
}
