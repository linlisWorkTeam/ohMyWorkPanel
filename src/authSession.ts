export type AuthUser = {
  userId: string;
  username: string;
  isAdmin: boolean;
};

const KEY = "ohmyworkpanel_auth_user";

export function loadAuthUser(): AuthUser | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return null;
    const v = JSON.parse(raw) as AuthUser;
    if (!v?.userId) return null;
    return v;
  } catch {
    return null;
  }
}

export function saveAuthUser(user: AuthUser | null): void {
  try {
    if (!user) localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, JSON.stringify(user));
  } catch {
    /* ignore */
  }
}

/** Prefer linked user member; admins fall back to group owner. */
export function resolveSenderMemberId(
  members: { id: string; authUserId?: string | null; kind: string }[],
  ownerMemberId: string,
  authUserId: string | null | undefined,
  isAdmin: boolean,
): string | null {
  if (authUserId) {
    const mine = members.find((m) => m.authUserId === authUserId && m.kind === "user");
    if (mine) return mine.id;
  }
  if (isAdmin) return ownerMemberId || null;
  return null;
}
