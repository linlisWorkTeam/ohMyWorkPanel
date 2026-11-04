//! 聊天状态归并（M1：消息与 run 的事实归 chat 领域）。
//!
//! 纯函数：输入当前 GroupState 与一条 WS 事件，返回新状态与「是否需要重拉」。
//! App 只负责取快照与提示错误，不再内联合并细节。

import { appendChannelDelta, isLazyMessageChannel } from "./messageContent";
import type { ChatEvent, GroupState } from "../types";

export type MergeOutcome = {
  state: GroupState | null;
  /** 事件对应的消息/run 不在当前快照里，调用方应重拉。 */
  missing: boolean;
};

function inCurrent(state: GroupState | null, payload: ChatEvent): boolean {
  if (!state) return false;
  if (!payload.groupId) return true;
  if (payload.groupId === state.group.id) return true;
  return Boolean(payload.runId && state.runs.some((run) => run.id === payload.runId));
}

/** 套用一条 `message_delta`。未知消息返回 missing，由调用方刷新。 */
export function applyMessageDelta(previous: GroupState | null, payload: ChatEvent): MergeOutcome {
  if (payload.kind !== "message_delta" || !payload.messageId) {
    return { state: previous, missing: false };
  }
  if (!previous || !inCurrent(previous, payload)) {
    return { state: previous, missing: false };
  }
  const channel = payload.channel ?? "final";
  const lazy = isLazyMessageChannel(channel);
  if (!lazy && !payload.delta) return { state: previous, missing: false };

  const index = previous.messages.findIndex((message) => message.id === payload.messageId);
  if (index < 0) return { state: previous, missing: true };

  const messages = previous.messages.slice();
  const message = messages[index];
  if (lazy) {
    messages[index] = {
      ...message,
      hasThinking:
        channel === "thinking" || channel === "reasoning" || channel === "thought"
          ? true
          : message.hasThinking,
      hasArtifact:
        ["artifact", "tool", "tool_result", "command"].includes(channel)
          ? true
          : message.hasArtifact,
      status: "streaming",
    };
  } else {
    messages[index] = {
      ...message,
      content: appendChannelDelta(message.content, channel, payload.delta ?? "", Boolean(payload.replace)),
      status: payload.status === "completed" ? "completed" : "streaming",
    };
  }
  return { state: { ...previous, messages }, missing: false };
}

const TERMINAL_RUN_STATUSES = ["completed", "failed", "cancelled", "interrupted"];

export function isTerminalRunStatus(status: string | null | undefined): boolean {
  return TERMINAL_RUN_STATUSES.includes(String(status ?? ""));
}

/** 套用一条 `run_status`；终态会同步把输出消息从 streaming 收尾。 */
export function applyRunStatus(previous: GroupState | null, payload: ChatEvent): MergeOutcome {
  if (payload.kind !== "run_status" || !payload.runId) {
    return { state: previous, missing: false };
  }
  if (!previous || !inCurrent(previous, payload)) {
    return { state: previous, missing: false };
  }
  const index = previous.runs.findIndex((run) => run.id === payload.runId);
  if (index < 0) return { state: previous, missing: true };

  const runs = previous.runs.slice();
  const run = runs[index];
  runs[index] = {
    ...run,
    status: (payload.status as typeof run.status) ?? run.status,
    outputMessageId: payload.messageId ?? run.outputMessageId,
    errorMessage: payload.error ?? run.errorMessage,
    phase: payload.phase ?? run.phase,
    phaseUpdatedAt: Date.now(),
  };

  let messages = previous.messages;
  const outputId = runs[index].outputMessageId;
  const terminal = isTerminalRunStatus(payload.status);
  if (terminal && outputId) {
    const messageIndex = messages.findIndex((message) => message.id === outputId);
    if (messageIndex >= 0 && messages[messageIndex].status === "streaming") {
      messages = messages.slice();
      messages[messageIndex] = {
        ...messages[messageIndex],
        status: payload.status === "completed" ? "completed" : (payload.status ?? messages[messageIndex].status),
      };
    }
  }
  return { state: { ...previous, runs, messages }, missing: false };
}
