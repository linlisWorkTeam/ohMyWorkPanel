//! Goal 证据账本的纯函数投影：把轮次聚合成可读统计与标签。

import type { GoalRound, GoalRoundVerdict } from "../types";

export interface RoundStats {
  total: number;
  met: number;
  progress: number;
  noProgress: number;
  failed: number;
  tokens: number;
}

const VERDICT_LABEL: Record<GoalRoundVerdict, string> = {
  goal_met: "达成",
  progress: "进展",
  no_progress: "无进展",
  failed: "失败",
};

export function verdictLabel(verdict: GoalRoundVerdict): string {
  return VERDICT_LABEL[verdict] ?? String(verdict);
}

/** 汇总轮次：各判定计数 + token 总量。 */
export function summarizeRounds(rounds: readonly GoalRound[]): RoundStats {
  const stats: RoundStats = {
    total: rounds.length,
    met: 0,
    progress: 0,
    noProgress: 0,
    failed: 0,
    tokens: 0,
  };
  for (const round of rounds) {
    stats.tokens += round.tokensUsed;
    switch (round.verdict) {
      case "goal_met":
        stats.met += 1;
        break;
      case "progress":
        stats.progress += 1;
        break;
      case "no_progress":
        stats.noProgress += 1;
        break;
      case "failed":
        stats.failed += 1;
        break;
      default:
        break;
    }
  }
  return stats;
}

/** 一行账本的紧凑描述：`#2 进展 · 128 tok · 摘要`。 */
export function formatRoundLine(round: GoalRound, maxSummary = 80): string {
  const summary = round.summary.trim();
  const clipped = summary.length > maxSummary ? `${summary.slice(0, maxSummary)}…` : summary;
  const tokens = ` · ${round.tokensUsed} tok`;
  return clipped ? `#${round.index} ${verdictLabel(round.verdict)}${tokens} · ${clipped}` : `#${round.index} ${verdictLabel(round.verdict)}${tokens}`;
}
