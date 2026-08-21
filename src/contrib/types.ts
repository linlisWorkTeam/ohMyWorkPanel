export type ContribSlot =
  | "right-tab"
  | "right-dock"
  | "composer-tool"
  | "status"
  | "message-action"
  | "member-action";

export type ContribOrigin = "base" | "extend";

export type ContribMotion = "none" | "enter" | "ambient";

/** Data-only UI contribution. Render is injected by the host (App), not stored here. */
export type UiContribution = {
  id: string;
  title: string;
  slot: ContribSlot;
  origin: ContribOrigin;
  order?: number;
  motion?: ContribMotion;
  dockable?: boolean;
};
