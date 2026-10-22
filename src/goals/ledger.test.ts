import { describe, expect, it } from "vitest";

import { formatRoundLine, summarizeRounds, verdictLabel } from "./ledger";
import type { GoalRound, GoalRoundVerdict } from "../types";

function round(index: number, verdict: GoalRoundVerdict, tokensUsed: number, summary = ""): GoalRound {
  return { index, verdict, tokensUsed, finishedAt: 0, summary };
}

describe("summarizeRounds", () => {
  it("counts verdicts and sums tokens", () => {
    const stats = summarizeRounds([
      round(0, "progress", 10),
      round(1, "progress", 20),
      round(2, "no_progress", 5),
      round(3, "failed", 7),
      round(4, "goal_met", 100),
    ]);

    expect(stats).toEqual({
      total: 5,
      met: 1,
      progress: 2,
      noProgress: 1,
      failed: 1,
      tokens: 142,
    });
  });

  it("handles an empty ledger", () => {
    expect(summarizeRounds([])).toEqual({
      total: 0,
      met: 0,
      progress: 0,
      noProgress: 0,
      failed: 0,
      tokens: 0,
    });
  });
});

describe("verdictLabel", () => {
  it("maps every verdict to a label", () => {
    expect(verdictLabel("goal_met")).toBe("达成");
    expect(verdictLabel("progress")).toBe("进展");
    expect(verdictLabel("no_progress")).toBe("无进展");
    expect(verdictLabel("failed")).toBe("失败");
  });
});

describe("formatRoundLine", () => {
  it("formats index, verdict, tokens and summary", () => {
    expect(formatRoundLine(round(2, "progress", 128, "完成了登录页"))).toBe(
      "#2 进展 · 128 tok · 完成了登录页",
    );
  });

  it("omits the separator when the summary is empty", () => {
    expect(formatRoundLine(round(0, "no_progress", 3))).toBe("#0 无进展 · 3 tok");
  });

  it("clips long summaries", () => {
    const long = "x".repeat(120);
    const line = formatRoundLine(round(1, "progress", 1, long), 10);

    expect(line).toContain("xxxxxxxxxx…");
    expect(line.length).toBeLessThan(long.length);
  });
});
