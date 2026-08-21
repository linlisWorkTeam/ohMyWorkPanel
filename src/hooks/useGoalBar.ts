import { useEffect, useState } from "react";
import { api } from "../api";

export type GoalBarState = {
  versionName: string;
  waveTitle: string;
  done: number;
  total: number;
  status: string;
};

/**
 * 项目群 goal bar（P2）：把当前版本 / Wave 上移为 chat 常驻条。
 * 仅项目群（groupKind !== "chat"）；无版本/非项目群时返回 null，失败安静回落。
 * 数据来自 api.getVersionBoard（复用既有接口）。
 */
export function useGoalBar(group: { id: string; groupKind?: string } | undefined): GoalBarState | null {
  const [goalBar, setGoalBar] = useState<GoalBarState | null>(null);
  useEffect(() => {
    const gid = group?.id;
    const isProject = group?.groupKind !== "chat";
    setGoalBar(null);
    if (!gid || !isProject) return;
    let cancelled = false;
    api
      .getVersionBoard(gid)
      .then((board) => {
        if (cancelled) return;
        const versions = [...(board.versions ?? [])].sort((a, b) => b.createdAt - a.createdAt);
        const version = versions[0];
        if (!version) {
          setGoalBar(null);
          return;
        }
        const waves = (board.waves ?? []).filter((w) => w.versionId === version.id).sort((a, b) => a.idx - b.idx);
        if (waves.length === 0) {
          setGoalBar(null);
          return;
        }
        const activeWave = waves.find((w) => w.status === "running" || w.status === "paused") ?? waves[waves.length - 1];
        setGoalBar({
          versionName: version.name,
          waveTitle: activeWave.title || `Wave ${activeWave.idx}`,
          done: waves.filter((w) => w.status === "done" || w.status === "skipped").length,
          total: waves.length,
          status: activeWave.status,
        });
      })
      .catch(() => {
        if (!cancelled) setGoalBar(null);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [group?.id, group?.groupKind]);
  return goalBar;
}
