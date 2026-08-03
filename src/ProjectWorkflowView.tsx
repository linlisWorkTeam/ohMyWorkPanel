import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "./api";
import { PmPanel } from "./PmPanel";
import type { Group, Member, OpsJobState, ReleaseStatus, RoadmapItem, TaskRun } from "./types";

interface Props {
  group: Group;
  members: Member[];
  runs: TaskRun[];
  canManage: boolean;
  onGroupPatch: (group: Group) => void;
  onError: (msg: string) => void;
}

const STATUS_LABEL: Record<string, string> = {
  backlog: "待办",
  in_progress: "进行中",
  review: "评审",
  done: "完成",
};

export function ProjectWorkflowView({ group, members, runs, canManage, onGroupPatch, onError }: Props) {
  const [roadmap, setRoadmap] = useState<RoadmapItem[]>([]);
  const [announcement, setAnnouncement] = useState(group.announcement ?? "");
  const [savingAnn, setSavingAnn] = useState(false);
  const [checklist, setChecklist] = useState(() => loadChecklist(group.id));
  const [release, setRelease] = useState<ReleaseStatus | null>(null);
  const [job, setJob] = useState<OpsJobState | null>(null);

  const refreshRoadmap = useCallback(async () => {
    try {
      const state = await api.getRoadmapState(group.id);
      setRoadmap(state.items);
    } catch (e: unknown) {
      onError(e instanceof Error ? e.message : String(e));
    }
  }, [group.id, onError]);

  useEffect(() => { void refreshRoadmap(); }, [refreshRoadmap]);
  useEffect(() => { setAnnouncement(group.announcement ?? ""); }, [group.id, group.announcement]);

  useEffect(() => {
    let alive = true;
    const tick = async () => {
      try {
        const [r, j] = await Promise.all([api.opsReleaseStatus(), api.opsJob()]);
        if (!alive) return;
        setRelease(r);
        setJob(j);
      } catch {
        /* ops optional on some hosts */
      }
    };
    void tick();
    const id = window.setInterval(() => {
      if (job?.running) void tick();
    }, 2000);
    return () => { alive = false; window.clearInterval(id); };
  }, [group.id, job?.running]);

  const currentStep = useMemo(() => {
    const ordered = [...roadmap].sort((a, b) => a.sortOrder - b.sortOrder);
    return ordered.find((i) => i.status === "in_progress")
      ?? ordered.find((i) => i.status !== "done")
      ?? ordered[ordered.length - 1]
      ?? null;
  }, [roadmap]);

  const agents = members.filter((m) => m.kind === "agent" && m.isActive);
  const activeRuns = runs.filter((r) =>
    ["queued", "running", "awaiting_review", "changes_requested"].includes(r.status)
  );

  const saveAnnouncement = async () => {
    setSavingAnn(true);
    try {
      const g = await api.setGroupAnnouncement(group.id, announcement);
      onGroupPatch({ ...group, ...g, announcement: g.announcement ?? announcement });
    } catch (e: unknown) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setSavingAnn(false);
    }
  };

  const toggleCheck = (key: string) => {
    const next = { ...checklist, [key]: !checklist[key] };
    setChecklist(next);
    saveChecklist(group.id, next);
  };

  const runGate = async () => {
    try {
      await api.opsRunTestGate();
      setJob(await api.opsJob());
    } catch (e: unknown) {
      onError(e instanceof Error ? e.message : String(e));
    }
  };

  const deployCanary = async () => {
    if (!confirm("确认部署到灰度 canary？将先跑测试门禁，不改动生产数据。")) return;
    try {
      await api.opsDeployCanary();
      setJob(await api.opsJob());
      setRelease(await api.opsReleaseStatus());
    } catch (e: unknown) {
      onError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="project-workflow">
      <section className="wf-section">
        <h2>群公告（项目级规则）</h2>
        <p className="wf-hint">保存后注入所有 Agent 的 prompt，并同步到工作区 `.cursor/rules/group-announcement.mdc`。</p>
        <textarea
          className="wf-announce"
          rows={4}
          value={announcement}
          disabled={!canManage}
          onChange={(e) => setAnnouncement(e.target.value)}
          placeholder="例如：commit 前必须跑 pnpm run test:gate；只部署灰度…"
        />
        {canManage && (
          <button className="pm-btn primary sm" disabled={savingAnn} onClick={() => void saveAnnouncement()}>
            {savingAnn ? "保存中…" : "保存公告"}
          </button>
        )}
      </section>

      <section className="wf-section">
        <h2>Roadmap 进度</h2>
        {roadmap.length === 0 ? (
          <p className="wf-hint">尚未创建路线图项 — 在下方看板管理中添加。</p>
        ) : (
          <div className="roadmap-strip">
            {[...roadmap].sort((a, b) => a.sortOrder - b.sortOrder).map((item) => {
              const current = currentStep?.id === item.id;
              return (
                <div key={item.id} className={`roadmap-step ${item.status} ${current ? "current" : ""}`}>
                  <span className="step-status">{STATUS_LABEL[item.status] ?? item.status}</span>
                  <strong>{item.title}</strong>
                  {current && <em>当前</em>}
                </div>
              );
            })}
          </div>
        )}
        <p className="wf-meta">工作区：<code>{group.workspacePath}</code></p>
      </section>

      <section className="wf-section">
        <h2>Agent 当前任务</h2>
        <div className="agent-lanes">
          {agents.map((agent) => {
            const mine = activeRuns.filter((r) => r.agentMemberId === agent.id);
            return (
              <div key={agent.id} className="agent-lane">
                <div className="lane-head">
                  <span className="lane-dot" style={{ background: agent.avatarColor }} />
                  <strong>{agent.displayName}</strong>
                  <small>{agent.adapter ?? "—"}</small>
                </div>
                {mine.length === 0 ? (
                  <div className="lane-idle">空闲</div>
                ) : (
                  mine.map((run) => (
                    <div key={run.id} className={`lane-run ${run.status}`}>
                      <span>{run.status}</span>
                      <code>{run.id.slice(0, 8)}</code>
                    </div>
                  ))
                )}
              </div>
            );
          })}
          {agents.length === 0 && <p className="wf-hint">暂无 Agent 成员</p>}
        </div>
      </section>

      <section className="wf-section ops-section">
        <h2>质量与发布</h2>
        <ul className="ops-checklist">
          <li>
            <label>
              <input type="checkbox" checked={!!checklist.design} onChange={() => toggleCheck("design")} />
              已复核自动化测试设计（行为变更有对应用例）
            </label>
          </li>
          <li>
            <label>
              <input type="checkbox" checked={!!checklist.gate} onChange={() => toggleCheck("gate")} />
              本地/服务器已跑通测试门禁
            </label>
          </li>
        </ul>
        <div className="ops-actions">
          <button className="pm-btn primary sm" disabled={!!job?.running || !canManage} onClick={() => void runGate()}>
            跑测试门禁
          </button>
          <button className="pm-btn sm" disabled={!!job?.running || !canManage} onClick={() => void deployCanary()}>
            部署灰度
          </button>
          <button className="pm-btn sm" type="button" onClick={() => void api.opsReleaseStatus().then(setRelease).catch((e) => onError(String(e)))}>
            刷新状态
          </button>
        </div>
        {release && (
          <div className="ops-status">
            <div>生产 :{release.prod.port} → {release.prod.httpStatus ?? "—"} · {release.prod.dataDir}</div>
            <div>灰度 :{release.canary.port} → {release.canary.httpStatus ?? "—"} · {release.canary.dataDir}</div>
            <div className="wf-hint">{release.note}</div>
          </div>
        )}
        {job && (job.running || job.log) && (
          <pre className="ops-log">{job.running ? `[运行中: ${job.kind}]\n` : `[结束: ${job.kind} exit=${job.exitCode}]\n`}{job.log.slice(-8000)}</pre>
        )}
      </section>

      <section className="wf-section wf-board">
        <h2>工作流看板</h2>
        <PmPanel groupId={group.id} members={members} onError={onError} />
      </section>
    </div>
  );
}

function loadChecklist(groupId: string): Record<string, boolean> {
  try {
    return JSON.parse(sessionStorage.getItem(`linlis-ops-check-${groupId}`) ?? "{}") as Record<string, boolean>;
  } catch {
    return {};
  }
}

function saveChecklist(groupId: string, value: Record<string, boolean>) {
  try {
    sessionStorage.setItem(`linlis-ops-check-${groupId}`, JSON.stringify(value));
  } catch { /* ignore */ }
}
