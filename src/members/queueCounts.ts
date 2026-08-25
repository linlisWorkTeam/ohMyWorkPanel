import type { TaskRun } from "../types";

export type QueueCounts = { running: number; queued: number };

/** Aggregate active runs for one agent (others ignored). */
export function queueCounts(runs: TaskRun[], agentMemberId: string): QueueCounts {
  let running = 0;
  let queued = 0;
  for (const run of runs) {
    if (run.agentMemberId !== agentMemberId) continue;
    if (run.status === "running") running += 1;
    else if (run.status === "queued") queued += 1;
  }
  return { running, queued };
}

/** Busy status fragment for member panel; null means show idle runtime text. */
export function agentBusyLabel(counts: QueueCounts): string | null {
  if (counts.running > 0) {
    return counts.queued > 0 ? `执行中 · 排队 ${counts.queued}` : "执行中";
  }
  if (counts.queued > 0) return `排队 ${counts.queued}`;
  return null;
}

/** Active runs for an agent, oldest first (running then queued by createdAt). */
export function runsForAgentActive(runs: TaskRun[], agentMemberId: string): TaskRun[] {
  return runs
    .filter(
      (run) =>
        run.agentMemberId === agentMemberId &&
        (run.status === "queued" || run.status === "running"),
    )
    .slice()
    .sort((a, b) => a.createdAt - b.createdAt);
}
