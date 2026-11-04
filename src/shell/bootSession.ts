//! 启动编排（M1：把 bootstrap 顺序从 App effect 中拆出）。
//!
//! 只按顺序调用 ctx 回调；失败向上抛，由调用方决定登录或报错。

import type { AgentModelsPayload } from "../agents/agentModels";
import type { AuthUser } from "../accounts/authSession";
import type { Group, PresetRole, RuntimeSettings } from "../types";

export type BootApi = {
  verify?: () => Promise<{ sub: string; username: string; isAdmin?: boolean; is_admin?: boolean }>;
  bootstrap: () => Promise<{ groups: Group[] }>;
  listPresence?: () => Promise<{ onlineUserIds?: string[] }>;
  getSettings: () => Promise<RuntimeSettings>;
  getAgentModels?: () => Promise<AgentModelsPayload>;
  getPresetRoles: () => Promise<PresetRole[]>;
};

export type BootOptions = {
  verify: boolean;
  presence: boolean;
  models: boolean;
};

export type BootContext = {
  api: BootApi;
  refresh: (groupId: string) => Promise<void>;
  isDisposed: () => boolean;
  onUser: (user: AuthUser) => void;
  onGroups: (groups: Group[]) => void;
  onOnline: (ids: string[]) => void;
  /** 选中第一个群（滚动锚点）。 */
  onFirstGroup: (groupId: string) => void;
  /** 第一个群快照加载完成（清未读）。 */
  onFirstGroupLoaded: (groupId: string) => void;
  onEmpty: () => void;
  onSettings: (settings: RuntimeSettings) => void;
  onModels?: (payload: AgentModelsPayload) => void;
  onPresetRoles: (roles: PresetRole[]) => void;
  onReady: () => void;
};

export async function bootSession(ctx: BootContext, options: BootOptions): Promise<void> {
  const { api } = ctx;
  if (options.verify && api.verify) {
    try {
      const me = await api.verify();
      if (!ctx.isDisposed()) {
        ctx.onUser({
          userId: me.sub,
          username: me.username,
          isAdmin: Boolean(me.isAdmin ?? me.is_admin),
        });
      }
    } catch {
      /* keep cached user */
    }
  }

  const boot = await api.bootstrap();
  if (ctx.isDisposed()) return;
  ctx.onGroups(boot.groups);

  if (options.presence && api.listPresence) {
    try {
      const presence = await api.listPresence();
      if (!ctx.isDisposed()) ctx.onOnline(presence.onlineUserIds ?? []);
    } catch {
      /* optional */
    }
  }

  const first = boot.groups[0];
  if (first) {
    ctx.onFirstGroup(first.id);
    await ctx.refresh(first.id);
    if (!ctx.isDisposed()) ctx.onFirstGroupLoaded(first.id);
  } else {
    ctx.onEmpty();
  }

  try {
    ctx.onSettings(await api.getSettings());
  } catch {
    /* scoped users may lack settings */
  }
  if (options.models && api.getAgentModels) {
    try {
      ctx.onModels?.(await api.getAgentModels());
    } catch {
      /* optional live catalog */
    }
  }
  try {
    ctx.onPresetRoles(await api.getPresetRoles());
  } catch {
    /* optional */
  }

  if (!ctx.isDisposed()) ctx.onReady();
}
