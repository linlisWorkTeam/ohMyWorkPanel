import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";
import { PmPanel } from "./PmPanel";
import { ServerPathPicker } from "../groups/ServerPathPicker";
import { boundWorkForItem, orchCursorLabel, orchDisplayLabel, orchStartBlockers, activeOrchAlerts, roadmapProgress, agentOrchLane, checklistPct } from "./roadmapUi";
import type {
  ChatEvent, Feature, FeatureTask, Group, Member, OpsJobState, ReleaseStatus,
  RoadmapItem, RoadmapOrchestration, TaskRun,
} from "../types";

interface Props {
  group: Group;
  members: Member[];
  runs: TaskRun[];
  canManage: boolean;
  onGroupPatch: (group: Group) => void;
  onMemberPatch: (member: Member) => void;
  onError: (msg: string) => void;
}

const PHASE_LABEL: Record<string, string> = {
  queued: "排队", starting: "启动", preparing: "准备", cli_spawn: "拉起 CLI",
  awaiting_first_token: "等待首包", streaming: "流式输出", finalizing: "收尾",
  completed: "完成", failed: "失败",
};

const STATUS_LABEL: Record<string, string> = {
  backlog: "待办",
  in_progress: "进行中",
  review: "评审",
  done: "完成",
};

export function ProjectWorkflowView({ group, members, runs, canManage, onGroupPatch, onMemberPatch, onError }: Props) {
  const [roadmap, setRoadmap] = useState<RoadmapItem[]>([]);
  const [features, setFeatures] = useState<Feature[]>([]);
  const [tasks, setTasks] = useState<FeatureTask[]>([]);
  const [orchestrations, setOrchestrations] = useState<RoadmapOrchestration[]>([]);
  const [orchBusy, setOrchBusy] = useState<string | null>(null);
  const [announcement, setAnnouncement] = useState(group.announcement ?? "");
  const [savingAnn, setSavingAnn] = useState(false);
  const [checklist, setChecklist] = useState(() => loadChecklist(group.id));
  const [release, setRelease] = useState<ReleaseStatus | null>(null);
  const [job, setJob] = useState<OpsJobState | null>(null);
  const [groupWs, setGroupWs] = useState(group.workspacePath);
  const [savingWs, setSavingWs] = useState(false);
  const [editingAgent, setEditingAgent] = useState<string | null>(null);
  const [agentWs, setAgentWs] = useState("");

  useEffect(() => { setGroupWs(group.workspacePath); }, [group.id, group.workspacePath]);

  const refreshRoadmap = useCallback(async () => {
    try {
      const [state, orch] = await Promise.all([
        api.getRoadmapState(group.id),
        api.listRoadmapOrchestrations(group.id).catch(() => [] as RoadmapOrchestration[]),
      ]);
      setRoadmap(state.items);
      setFeatures(state.features);
      setTasks(state.tasks);
      setOrchestrations(orch);
    } catch (e: unknown) {
      onError(e instanceof Error ? e.message : String(e));
    }
  }, [group.id, onError]);

  useEffect(() => { void refreshRoadmap(); }, [refreshRoadmap]);

  // Instant refresh when orchestrator advances (WS), not only the 4s poll.
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<ChatEvent>("chat-event", (event) => {
      if (disposed) return;
      const payload = event.payload;
      if (payload.kind !== "orchestration_status") return;
      if (payload.groupId && payload.groupId !== group.id) return;
      void refreshRoadmap();
    }).then((fn) => { unlisten = fn; });
    return () => { disposed = true; unlisten?.(); };
  }, [group.id, refreshRoadmap]);

  const orchByItem = useMemo(() => {
    const map = new Map<string, RoadmapOrchestration>();
    for (const o of orchestrations) {
      if (!map.has(o.roadmapItemId) || (o.status === "running" || o.status === "paused" || o.status === "failed")) {
        map.set(o.roadmapItemId, o);
      }
    }
    return map;
  }, [orchestrations]);

  const runOrch = async (label: string, action: () => Promise<unknown>) => {
    setOrchBusy(label);
    try {
      await action();
      await refreshRoadmap();
    } catch (e: unknown) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setOrchBusy(null);
    }
  };
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

  const progress = useMemo(() => roadmapProgress(roadmap), [roadmap]);
  const orchAlerts = useMemo(() => activeOrchAlerts(roadmap, orchByItem), [roadmap, orchByItem]);

  const agents = members.filter((m) => (m.kind === "agent" || m.kind === "chatbot") && m.isActive);
  const activeRuns = runs.filter((r) =>
    ["queued", "running", "awaiting_review", "changes_requested"].includes(r.status)
  );

  // Keep orchestration badges fresh while agents are working.
  useEffect(() => {
    const active = orchestrations.some((o) => o.status === "running" || o.status === "paused" || o.status === "failed");
    if (!active && activeRuns.length === 0) return;
    const id = window.setInterval(() => { void refreshRoadmap(); }, 4000);
    return () => window.clearInterval(id);
  }, [orchestrations, activeRuns.length, refreshRoadmap]);

  const saveGroupWorkspace = async () => {
    setSavingWs(true);
    try {
      const g = await api.updateGroupWorkspace(group.id, groupWs);
      onGroupPatch({ ...group, ...g });
    } catch (e: unknown) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setSavingWs(false);
    }
  };

  const saveAgentWorkspace = async (memberId: string) => {
    try {
      const m = await api.updateMemberWorkspace(memberId, agentWs);
      onMemberPatch(m);
      setEditingAgent(null);
    } catch (e: unknown) {
      onError(e instanceof Error ? e.message : String(e));
    }
  };

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

      <section className="wf-section wf-roadmap">
        <div className="wf-section-head">
          <h2>Roadmap 进度</h2>
          {roadmap.length > 0 && (
            <div className="roadmap-meter" aria-label={`完成 ${progress.done}/${progress.total}`}>
              <div className="roadmap-meter-track">
                <div className="roadmap-meter-fill" style={{ width: `${progress.pct}%` }} />
              </div>
              <span className="roadmap-meter-label">{progress.done}/{progress.total} · {progress.pct}%</span>
            </div>
          )}
        </div>
        {orchAlerts.length > 0 && (
          <div className="orch-alert-stack" role="status">
            {orchAlerts.map((a) => (
              <div key={a.itemTitle + a.message} className="orch-alert">
                <strong>{a.itemTitle}</strong>
                <span>{a.message}</span>
              </div>
            ))}
          </div>
        )}
        {roadmap.length === 0 ? (
          <div className="wf-empty">
            <strong>还没有路线图</strong>
            <p>在下方「工作流看板」添加路线图项，并把功能/checklist 绑定到该项，即可一键让 Agent 串行执行。</p>
          </div>
        ) : (
          <div className="roadmap-strip" role="list">
            {[...roadmap].sort((a, b) => a.sortOrder - b.sortOrder).map((item, index, arr) => {
              const current = currentStep?.id === item.id;
              const orch = orchByItem.get(item.id);
              const busy = orchBusy === item.id;
              const work = boundWorkForItem(item.id, features, tasks);
              const cursor = orchCursorLabel(orch, features, tasks);
              const blockers = orchStartBlockers(item.id, features, tasks, Boolean(group.adminMemberId));
              const canStart = blockers.length === 0;
              const display = orch ? orchDisplayLabel(orch) : null;
              const orchLive = orch && (orch.status === "running" || orch.status === "paused" || orch.status === "failed");
              return (
                <div key={item.id} className="roadmap-step-wrap" role="listitem">
                  <div className={`roadmap-step ${item.status} ${current ? "current" : ""} ${orch?.status === "running" ? "orch-running" : ""} ${display?.tone === "failed" ? "orch-failed" : ""}`}>
                    <span className="step-status">{STATUS_LABEL[item.status] ?? item.status}</span>
                    <strong>{item.title}</strong>
                    {current && <em className="step-now">当前</em>}
                    <small className="step-bound">
                      {work.totalTasks === 0
                        ? "未绑定 checklist"
                        : `checklist ${work.doneTasks}/${work.totalTasks} · ${checklistPct(work.doneTasks, work.totalTasks)}%`}
                    </small>
                    {work.totalTasks > 0 && (
                      <div className="step-mini-meter" aria-hidden>
                        <div
                          className={`step-mini-fill ${item.status === "done" ? "done" : ""}`}
                          style={{ width: `${checklistPct(work.doneTasks, work.totalTasks)}%` }}
                        />
                      </div>
                    )}
                    {!canStart && item.status !== "done" && (!orch || orch.status === "completed" || orch.status === "cancelled") && (
                      <small className="orch-hint orch-failed">{blockers[0]}</small>
                    )}
                    {orchLive && display && (
                      <small className={`orch-hint orch-${display.tone}`}>
                        {display.label}
                        {cursor ? ` · ${cursor}` : ""}
                        {orch.errorMessage && display.tone === "failed" ? ` · ${orch.errorMessage}` : ""}
                        {orch.errorMessage && display.tone === "paused" && orch.errorMessage !== "已手动暂停。" ? ` · ${orch.errorMessage}` : ""}
                      </small>
                    )}
                    {canManage && item.status !== "done" && (
                      <div className="orch-actions">
                        {(!orch || orch.status === "completed" || orch.status === "cancelled") && (
                          <button
                            type="button"
                            className="pm-btn primary sm"
                            disabled={!!orchBusy || !canStart}
                            title={canStart ? "启动 Agent 编排" : blockers.join("；")}
                            onClick={() => void runOrch(item.id, () => api.startRoadmapItem(item.id))}
                          >
                            {busy ? "…" : "启动"}
                          </button>
                        )}
                        {orch?.status === "running" && (
                          <>
                            <button
                              type="button"
                              className="pm-btn sm"
                              disabled={!!orchBusy}
                              onClick={() => void runOrch(item.id, () => api.pauseRoadmapOrchestration(orch.id))}
                            >
                              暂停
                            </button>
                            <button
                              type="button"
                              className="pm-btn sm"
                              disabled={!!orchBusy}
                              onClick={() => void runOrch(item.id, () => api.cancelRoadmapOrchestration(orch.id))}
                            >
                              取消
                            </button>
                          </>
                        )}
                        {(orch?.status === "paused" || orch?.status === "failed") && (
                          <>
                            <button
                              type="button"
                              className="pm-btn primary sm"
                              disabled={!!orchBusy}
                              onClick={() => void runOrch(item.id, () => api.resumeRoadmapOrchestration(orch.id))}
                            >
                              继续
                            </button>
                            <button
                              type="button"
                              className="pm-btn sm"
                              disabled={!!orchBusy}
                              onClick={() => void runOrch(item.id, () => api.cancelRoadmapOrchestration(orch.id))}
                            >
                              取消
                            </button>
                          </>
                        )}
                      </div>
                    )}
                  </div>
                  {index < arr.length - 1 && <div className={`roadmap-connector ${item.status === "done" ? "done" : ""}`} aria-hidden />}
                </div>
              );
            })}
          </div>
        )}
        <div className="wf-workspace">
          <p className="wf-meta">群工作区：<code>{group.workspacePath}</code></p>
          {canManage && (
            <div className="wf-ws-edit">
              <ServerPathPicker value={groupWs} onChange={setGroupWs} onError={onError} />
              <button className="pm-btn sm" disabled={savingWs || groupWs === group.workspacePath} onClick={() => void saveGroupWorkspace()}>
                {savingWs ? "保存中…" : "更新群工作区"}
              </button>
            </div>
          )}
        </div>
      </section>

      <section className="wf-section">
        <h2>Agent 当前任务</h2>
        <div className="agent-lanes">
          {agents.map((agent) => {
            const mine = activeRuns.filter((r) => r.agentMemberId === agent.id);
            const orchLane = agentOrchLane(agent.id, orchestrations, roadmap, features, tasks, runs);
            return (
              <div key={agent.id} className={`agent-lane ${orchLane ? `tone-${orchLane.tone}` : ""}`}>
                <div className="lane-head">
                  <span className="lane-dot" style={{ background: agent.avatarColor }} />
                  <strong>{agent.displayName}</strong>
                  <small>{agent.kind === "chatbot" ? "chatbot" : agent.adapter ?? "—"}{agent.keepAlive ? " · 保活" : ""}{agent.warmStatus ? ` · ${agent.warmStatus}` : ""}</small>
                </div>
                {agent.kind === "agent" && (
                  <div className="lane-ws">
                    <code title={agent.workspacePath ?? ""}>{agent.workspacePath ?? "（默认沙箱）"}</code>
                    {canManage && (
                      editingAgent === agent.id ? (
                        <div className="wf-ws-edit">
                          <ServerPathPicker value={agentWs} onChange={setAgentWs} onError={onError} />
                          <button className="pm-btn sm" onClick={() => void saveAgentWorkspace(agent.id)}>保存</button>
                          <button className="pm-btn sm quiet" onClick={() => setEditingAgent(null)}>取消</button>
                        </div>
                      ) : (
                        <button className="pm-btn sm quiet" onClick={() => { setEditingAgent(agent.id); setAgentWs(agent.workspacePath || group.workspacePath); }}>改工作区</button>
                      )
                    )}
                  </div>
                )}
                {orchLane && (
                  <div className={`lane-orch orch-${orchLane.tone}`}>
                    <span className="lane-orch-status">{orchLane.statusLabel}</span>
                    <strong>{orchLane.roadmapTitle}</strong>
                    {orchLane.cursor && <small>{orchLane.cursor}</small>}
                  </div>
                )}
                {mine.length === 0 && !orchLane ? (
                  <div className="lane-idle">空闲</div>
                ) : (
                  mine.map((run) => (
                    <div key={run.id} className={`lane-run ${run.status}`}>
                      <span>{run.phase ? (PHASE_LABEL[run.phase] ?? run.phase) : run.status}</span>
                      <code>{run.id.slice(0, 8)}</code>
                    </div>
                  ))
                )}
              </div>
            );
          })}
          {agents.length === 0 && <p className="wf-hint">暂无 Agent / 聊天机器人</p>}
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
        <PmPanel groupId={group.id} members={members} adminMemberId={group.adminMemberId} onError={onError} />
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
