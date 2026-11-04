//! WS 事件桥（M1：把 `listen("chat-event")` 的处理体从 App 拆出）。
//!
//! 只依赖注入的回调，不直接接触 React 状态；便于用假 deps 做契约测试。

import { applyPresence, shouldBumpUnread } from "../accounts/presence";
import { routeChatEvent } from "./eventRouting";
import {
  applyMessageDelta,
  applyRunHeartbeat,
  applyRunStatus,
  isTerminalRunStatus,
} from "./stateReducers";
import type { ChatEvent, GroupState } from "../types";

export type ChatEventBridgeDeps = {
  getActiveGroupId: () => string | null;
  setOnline: (updater: (previous: Set<string>) => Set<string>) => void;
  bumpUnread: (groupId: string) => void;
  setCurrent: (updater: (previous: GroupState | null) => GroupState | null) => void;
  /** 拉取某个群的快照；undefined 表示当前群。 */
  refresh: (groupId?: string) => Promise<void>;
  /** 重连后的全量重放（bootstrap + 当前群 + 活动 run）。 */
  resync: () => Promise<void>;
  /** 统一错误出口（401 → 登录，其余 → 错误提示）。 */
  reportError: (reason: unknown) => void;
};

export function dispatchChatEvent(payload: ChatEvent, deps: ChatEventBridgeDeps): void {
  const activeGroupId = deps.getActiveGroupId();
  const route = routeChatEvent(payload);

  if (route === "ignore") return;

  if (route === "presence") {
    deps.setOnline((previous) => applyPresence(previous, payload));
    return;
  }

  if (shouldBumpUnread(payload)) {
    deps.bumpUnread(payload.groupId!);
  }

  if (route === "orchestration") {
    if (activeGroupId) {
      void deps.refresh(activeGroupId).catch(deps.reportError);
    }
    return;
  }

  if (route === "resync") {
    void deps.resync().catch(deps.reportError);
    return;
  }

  if (payload.kind === "run_heartbeat" && payload.runId) {
    deps.setCurrent((previous) => applyRunHeartbeat(previous, payload));
    return;
  }

  if (payload.kind === "message_delta" && payload.messageId) {
    if (payload.groupId && payload.groupId !== activeGroupId) return;
    deps.setCurrent((previous) => {
      const outcome = applyMessageDelta(previous, payload);
      if (outcome.missing) {
        void deps.refresh(previous?.group.id ?? activeGroupId ?? undefined).catch(deps.reportError);
      }
      return outcome.state;
    });
    return;
  }

  if (payload.kind === "run_status" && payload.runId) {
    const terminal = isTerminalRunStatus(payload.status);
    deps.setCurrent((previous) => {
      const outcome = applyRunStatus(previous, payload);
      if (outcome.missing) {
        void deps.refresh(payload.groupId || previous?.group.id || undefined).catch(deps.reportError);
      }
      return outcome.state;
    });
    if (terminal) {
      void deps.refresh(payload.groupId || activeGroupId || undefined).catch(deps.reportError);
    }
    return;
  }

  void deps.refresh(payload.groupId || activeGroupId || undefined).catch(deps.reportError);
}
