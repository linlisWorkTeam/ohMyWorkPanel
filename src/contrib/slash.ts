/** Only commands the backend actually supports. Do not add /play /pause /advance /release. */
export const SLASH_COMMANDS = [
  { cmd: "/board", hint: "查看版本 / Wave 进度" },
  { cmd: "/approve", hint: "批准当前 Ask 版本（生成默认 Wave）" },
  { cmd: "/wave", hint: "重设当前版本 Wave：/wave <标题>" },
  { cmd: "/market", hint: "基于最近仓库进展生成五渠道宣传草稿" },
] as const;
