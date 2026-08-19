/** App.tsx 抽取的共享 UI 常量/工具（P1 组件化） */
export const PHASE_LABEL: Record<string, string> = {
  queued: "排队", starting: "启动", preparing: "准备", cli_spawn: "拉起 CLI",
  awaiting_first_token: "等待首包", streaming: "流式输出", finalizing: "收尾",
  completed: "完成", failed: "失败",
};
export const time = (value: number) => new Intl.DateTimeFormat("zh-CN", { hour: "2-digit", minute: "2-digit" }).format(value);
export const dayLabel = (value: number) => {
  const d = new Date(value);
  const today = new Date(); today.setHours(0, 0, 0, 0);
  const that = new Date(d); that.setHours(0, 0, 0, 0);
  const diff = Math.round((today.getTime() - that.getTime()) / 86400000);
  if (diff === 0) return "今天";
  if (diff === 1) return "昨天";
  return new Intl.DateTimeFormat("zh-CN", { month: "long", day: "numeric", weekday: "short" }).format(d);
};

export function readError(reason: unknown) { return typeof reason === "string" ? reason : reason instanceof Error ? reason.message : "发生了未知错误。"; }
