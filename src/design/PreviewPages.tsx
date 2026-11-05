//! 参考站各领域页面（1:1 结构，真实数据）。
//!
//! 所有类名来自 `.local-dev/ref/style.css`；无数据的区块按参考站空态降级。

import { useCallback, useEffect, useState } from "react";

import { api } from "../api";
import type { GoalBarState } from "../hooks/useGoalBar";
import type {
  ExtensionStatus, Goal, Group, LogEntry, Member, RuntimeSettings, TaskRun, VersionBoard,
} from "../types";
import type { ContentCampaign } from "../marketing/types";

export type PreviewPageProps = {
  group: Group | null;
  members: Member[];
  runs: TaskRun[];
  onlineUserIds: ReadonlySet<string> | null;
  senderMemberId: string | null;
  goalBar: GoalBarState | null;
  light: boolean;
  onToggleLight: () => void;
  onView: (view: string) => void;
  onMember: (member: Member) => void;
  onRefresh: () => Promise<void> | void;
};

function errText(reason: unknown): string {
  if (reason instanceof Error) return reason.message;
  return String(reason);
}

function Heading({ over, title, sub, action }: { over: string; title: string; sub: string; action?: React.ReactNode }) {
  return (
    <div className="pageHeading">
      <div>
        <div className="overline">{over}</div>
        <h2>{title}</h2>
        <p>{sub}</p>
      </div>
      {action}
    </div>
  );
}

function Stats({ items }: { items: Array<[string, string, string]> }) {
  return (
    <div className="cards">
      {items.map(([label, value, unit]) => (
        <div className="stat" key={label}>
          <span>{label}</span>
          <strong>{value}<small>{unit}</small></strong>
        </div>
      ))}
    </div>
  );
}

function PageError({ message }: { message: string }) {
  return <div className="panel"><p className="small" style={{ color: "var(--red)" }}>{message}</p></div>;
}

/* ---------------------------------------------------------------- 交付 */

export function DeliveryPage(props: PreviewPageProps) {
  const { group, senderMemberId, onView } = props;
  const [board, setBoard] = useState<VersionBoard | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    if (!group) return;
    try {
      setBoard(await api.getVersionBoard(group.id));
      setError(null);
    } catch (reason) {
      setError(errText(reason));
    }
  }, [group?.id]);

  useEffect(() => { void load(); }, [load]);

  const version = board?.versions.find((v) => v.status !== "released") ?? board?.versions[0] ?? null;
  const waves = board?.waves ?? [];
  const done = waves.filter((w) => w.status === "completed").length;
  const current = waves.find((w) => w.status !== "completed" && w.status !== "blocked") ?? null;

  const run = async (action: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await action();
      await load();
    } catch (reason) {
      setError(errText(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="pageScroll">
      <Heading
        over="DELIVERY / VERSION 2.2"
        title="让进展有迹可循"
        sub="从路线图到 Wave，每次推进都有明确的交付边界。"
        action={<button type="button" className="primary" disabled={busy} onClick={() => {
          const name = window.prompt("版本名称", `v${board?.versions.length ?? 1}.0`);
          if (!name) return;
          void run(() => api.createProjectVersion({ groupId: group?.id ?? "", name, mode: "create", requesterMemberId: senderMemberId ?? undefined }));
        }}>＋ 新建版本</button>}
      />
      <Stats items={[
        ["当前版本", version?.name ?? "—", version?.status ?? "未建立"],
        ["Wave 进度", `${done} / ${waves.length || 0}`, current ? "进行中" : "无进行中"],
        ["已发布", String(board?.versions.filter((v) => v.status === "released").length ?? 0), "个版本"],
      ]} />
      {error && <PageError message={error} />}
      {version ? (
        <div className="panel">
          <div className="panelHead">
            <h3>{version.name} <span className="tag green">{version.status === "released" ? "已发布" : "当前版本"}</span></h3>
            <small>Git · {board?.git?.headSha?.slice(0, 7) ?? (board?.git?.isGitRepo ? "Git 已连接" : "非 Git 仓库")}</small>
          </div>
          <p className="small muted">
            What · {version.what || "—"}<br />
            Who · {version.who || "—"}<br />
            How · {version.how || "—"}
          </p>
        </div>
      ) : (
        <div className="panel"><p className="small muted">尚未建立版本。新建版本后，这里会显示交付边界。</p></div>
      )}
      <div className="panel">
        <div className="panelHead"><h3>Wave 交付时间线</h3><small>{waves.length} 个阶段</small></div>
        <ol className="timeline">
          {waves.map((wave) => {
            const state = wave.status === "completed" ? "done" : wave === current ? "current" : "";
            return (
              <li key={wave.id} className={state}>
                <div className="waveTitle">
                  {String(wave.idx + 1).padStart(2, "0")} · {wave.title}
                  <span className={`tag ${wave.status === "completed" ? "green" : state === "current" ? "blue" : ""}`}>
                    {wave.status === "completed" ? "已完成" : state === "current" ? "进行中" : "待开始"}
                  </span>
                </div>
                <p>{wave.summary || "—"}</p>
                {state === "current" && (
                  <div className="buttonRow">
                    <button type="button" className="secondary" disabled={busy} onClick={() => void run(() => api.pauseWave(wave.id))}>Ⅱ 暂停 Wave</button>
                    <button type="button" className="primary" disabled={busy} onClick={() => void run(() => api.advanceWave(wave.id))}>推进下一阶段 →</button>
                  </div>
                )}
              </li>
            );
          })}
          {waves.length === 0 && <li><p className="small muted">当前版本还没有 Wave。</p></li>}
        </ol>
      </div>
      <div className="panel">
        <div className="panelHead" style={{ marginBottom: 0 }}>
          <div><h3>准备发布版本？</h3><small>发布版本记录，不等于部署到生产环境。</small></div>
          <button type="button" className="secondary" disabled={busy || !version} onClick={() => {
            if (!version || !window.confirm(`发布 ${version.name}？此操作只记录版本，不部署。`)) return;
            void run(() => api.releaseVersion(version.id));
          }}>发布确认 ↗</button>
        </div>
      </div>
      <p className="small muted" style={{ marginTop: 18 }}>↗ 需要管理 Wave 时，可在版本页继续；当前页面只呈现交付边界。 <button type="button" style={{ color: "var(--accent)", padding: 0 }} onClick={() => onView("versions")}>打开版本详情</button></p>
    </div>
  );
}

/* ---------------------------------------------------------------- 目标 */

const GOAL_LABEL: Record<string, string> = {
  active: "进行中",
  paused: "已暂停",
  awaiting_approval: "等待批准",
  aborted: "已停止",
};

export function GoalsPage(props: PreviewPageProps) {
  const { group, onView } = props;
  const [goals, setGoals] = useState<Goal[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const goal = goals[0] ?? null;

  const load = useCallback(async () => {
    if (!group) return;
    try {
      setGoals(await api.listGoals(group.id));
      setError(null);
    } catch (reason) {
      setError(errText(reason));
    }
  }, [group?.id]);

  useEffect(() => { void load(); }, [load]);

  const run = async (action: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await action();
      await load();
    } catch (reason) {
      setError(errText(reason));
    } finally {
      setBusy(false);
    }
  };

  const rounds = goal?.rounds.length ?? 0;
  const tokens = goal?.rounds.reduce((sum, round) => sum + round.tokensUsed, 0) ?? 0;
  const elapsedMin = goal?.startedAt ? Math.round((Date.now() - goal.startedAt) / 60000) : 0;
  const maxRounds = goal?.budget.maxRounds ?? 10;

  return (
    <div className="pageScroll">
      <Heading
        over="AUTONOMY / HUMAN IN THE LOOP"
        title="目标与证据"
        sub="给出目标与预算，在关键节点拍板。"
        action={<button type="button" className="secondary" onClick={() => onView("chat")}>＋ 新建目标（在群内）</button>}
      />
      {error && <PageError message={error} />}
      <div className="panel">
        <div className="panelHead">
          <span className={`tag ${goal?.status === "active" ? "green" : "amber"}`}>● {GOAL_LABEL[goal?.status ?? ""] ?? "未建立"}</span>
          <small>{goal ? `GOAL-${goal.id.slice(0, 4).toUpperCase()} · 自动推进 ${goal.autoAdvance ? "开" : "关"}` : "尚无目标"}</small>
        </div>
        <h3 style={{ fontWeight: 500, fontSize: 18, lineHeight: 1.8, margin: "12px 0" }}>
          {goal?.objective ?? "创建 Goal 后，这里会显示目标、预算与证据账本。"}
        </h3>
        <div className="budget">
          <div>已用轮次<strong>{rounds} <small>/ {maxRounds} 轮</small></strong><div className="progress"><i style={{ width: `${maxRounds ? (rounds / maxRounds) * 100 : 0}%` }} /></div></div>
          <div>Token 预算<strong>{(tokens / 1000).toFixed(1)}k <small>/ {Math.round((goal?.budget.maxTokens ?? 200000) / 1000)}k</small></strong><div className="progress"><i style={{ width: `${goal?.budget.maxTokens ? Math.min(100, (tokens / goal.budget.maxTokens) * 100) : 0}%` }} /></div></div>
          <div>运行时间<strong>{elapsedMin} min <small>/ {Math.round((goal?.budget.maxWallTimeMs ?? 3600000) / 60000)} min</small></strong><div className="progress"><i style={{ width: `${goal?.budget.maxWallTimeMs ? Math.min(100, ((elapsedMin * 60000) / goal.budget.maxWallTimeMs) * 100) : 0}%` }} /></div></div>
        </div>
        <div className="buttonRow">
          {goal ? (
            <>
              <button type="button" className="primary" disabled={busy} onClick={() => void run(() => goal.status === "awaiting_approval" ? api.approveGoal(goal.id) : goal.status === "paused" ? api.startGoal(goal.id) : api.advanceGoal(goal.id))}>
                {goal.status === "awaiting_approval" ? "✓ 批准本轮" : goal.status === "paused" ? "▶ 继续推进" : "推进一步 →"}
              </button>
              <button type="button" className="secondary" disabled={busy} onClick={() => void run(() => goal.status === "paused" ? api.startGoal(goal.id) : api.pauseGoal(goal.id))}>
                {goal.status === "paused" ? "继续" : "Ⅱ 暂停"}
              </button>
              <button type="button" className="secondary" disabled={busy} onClick={() => void run(() => api.abortGoal(goal.id))}>停止目标</button>
              <label className="pillSwitch" style={{ marginLeft: "auto" }}>
                <input type="checkbox" checked={goal.autoAdvance} onChange={(e) => void run(() => api.setGoalAutoAdvance(goal.id, e.target.checked))} />自动推进
              </label>
            </>
          ) : (
            <button type="button" className="primary" onClick={() => onView("chat")}>去群内创建目标 ↗</button>
          )}
        </div>
      </div>
      <div className="panel">
        <div className="panelHead"><h3>证据账本 <span className="tag">{rounds} 轮</span></h3><small>最近轮次优先</small></div>
        {[...(goal?.rounds ?? [])].reverse().slice(0, 7).map((round) => (
          <div className="round" key={round.index}>
            <span className="roundNum">#{String(round.index).padStart(2, "0")}</span>
            <div>
              <span className="tag green">{round.verdict}</span>
              <p className="small">{round.summary || "（无摘要）"}</p>
              <small className="muted">{(round.tokensUsed / 1000).toFixed(1)}k tokens · {new Date(round.finishedAt).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })}</small>
            </div>
          </div>
        ))}
        {rounds === 0 && <p className="small muted">还没有轮次记录。</p>}
      </div>
    </div>
  );
}

/* ---------------------------------------------------------------- 内容 */

const CAMPAIGN_LABEL: Record<string, string> = {
  collecting: "收集证据", planning: "规划中", writing: "写作中", validating: "校验中",
  awaiting_user: "待审批", changes_requested: "要求修改", no_content: "无内容", failed: "失败",
};

export function ContentPage(props: PreviewPageProps) {
  const { group, senderMemberId } = props;
  const [campaigns, setCampaigns] = useState<ContentCampaign[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const campaign = campaigns[0] ?? null;

  const load = useCallback(async () => {
    if (!group) return;
    try {
      setCampaigns(await api.listMarketingCampaigns(group.id));
      setError(null);
    } catch (reason) {
      setError(errText(reason));
    }
  }, [group?.id]);

  useEffect(() => { void load(); }, [load]);

  const run = async (action: () => Promise<unknown>) => {
    setBusy(true);
    try { await action(); await load(); } catch (reason) { setError(errText(reason)); } finally { setBusy(false); }
  };

  const draft = campaign?.drafts[0] ?? null;
  const brief = campaign?.brief ?? null;

  return (
    <div className="pageScroll">
      <Heading
        over="CONTENT / EVIDENCE FIRST"
        title="让成果被看见"
        sub="基于项目证据生成内容，经过批准再导出。"
        action={<button type="button" className="primary" onClick={() => window.alert("内容活动由群内 Agent 创建：在协作页 @ 规划 Agent 说明目标即可。")}>＋ 创建内容</button>}
      />
      {error && <PageError message={error} />}
      {campaign ? (
        <>
          <div className="panel">
            <div className="panelHead">
              <span className={`tag ${campaign.status === "awaiting_user" ? "amber" : "green"}`}>{CAMPAIGN_LABEL[campaign.status] ?? campaign.status}</span>
              <small>CAMPAIGN-{campaign.id.slice(0, 4).toUpperCase()}</small>
            </div>
            <h3 style={{ fontSize: 20, fontWeight: 500 }}>{brief?.coreMessage || draft?.title || "内容活动"}</h3>
            <p className="muted small">
              来源：{campaign.sourceMode} · {campaign.baseRef ?? "未指定"} · {campaign.headRef}<br />
              规划：{campaign.plannerAgentId.slice(0, 6)} / 写作：{campaign.writerAgentId.slice(0, 6)}
            </p>
            {draft && (
              <div style={{ borderLeft: "2px solid var(--accent)", padding: "0 0 0 16px", margin: "25px 0" }}>
                <p>{draft.title}</p>
                <p className="muted small">{draft.body.slice(0, 240)}</p>
              </div>
            )}
            {(campaign.validation ?? []).length > 0 && <p className="small" style={{ color: "var(--amber)" }}>校验提示：{campaign.validation[0].message}</p>}
            <div className="buttonRow">
              <button type="button" className="primary" disabled={busy} onClick={() => void run(() => api.approveMarketingCampaign(campaign.id, senderMemberId ?? ""))}>批准并导出导出包</button>
              <button type="button" className="secondary" disabled={busy} onClick={() => {
                const feedback = window.prompt("修改意见");
                if (feedback) void run(() => api.reviseMarketingCampaign(campaign.id, senderMemberId ?? "", feedback));
              }}>要求修改</button>
            </div>
          </div>
          <div className="panel">
            <div className="panelHead"><h3>导出</h3><small>{campaign.status === "awaiting_user" ? "批准后可导出" : "当前状态可尝试导出"}</small></div>
            <button type="button" className="secondary" disabled={busy} onClick={() => void run(async () => {
              const out = await api.exportMarketingCampaign(campaign.id);
              const blob = new Blob([out.markdown], { type: "text/markdown;charset=utf-8" });
              const url = URL.createObjectURL(blob);
              const anchor = document.createElement("a");
              anchor.href = url;
              anchor.download = out.filename;
              anchor.click();
              URL.revokeObjectURL(url);
            })}>导出 Markdown ↗</button>
          </div>
        </>
      ) : (
        <div className="panel"><p className="small muted">当前群还没有内容活动。在协作页 @ 规划 Agent 说明目标后会出现在这里。</p></div>
      )}
    </div>
  );
}

/* ---------------------------------------------------------------- 场景 */

export function ScenePage(props: PreviewPageProps) {
  const { members, runs, onMember, onView } = props;
  const agents = members.filter((m) => m.kind === "agent");
  const people = members.filter((m) => m.kind !== "agent");
  const active = runs.filter((run) => run.status === "running" || run.status === "queued");
  const working = new Set(active.map((run) => run.agentMemberId));

  return (
    <div className="pageScroll">
      <Heading
        over="AGENT PLAYGROUND / READ ONLY"
        title="大家都在忙什么"
        sub="每个群一个房间，场景只是协作状态的另一种表达。"
        action={<button type="button" className="secondary" onClick={() => onView("chat")}>返回协作 ↗</button>}
      />
      <Stats items={[["在场成员", String(members.length), "位"], ["正在工作", String(working.size), "位"], ["等待审批", String(runs.filter((r) => r.reviewStatus === "pending").length), "项"]]} />
      <div className="scene">
        <div className="zone">
          <h4>WORK ZONE / 工位区</h4>
          {agents.map((agent, index) => (
            <button type="button" key={agent.id} className="pixel" onClick={() => onMember(agent)}>
              <div className={`pixelPerson tone-${["a", "b", "c", "d"][index % 4]}`} />
              <small>{agent.displayName} · {working.has(agent.id) ? "工作中" : "空闲"}</small>
            </button>
          ))}
          {agents.length === 0 && <p className="small muted">还没有 Agent 成员。</p>}
        </div>
        <div className="zone">
          <h4>MEETING / 交流区</h4>
          {people.map((person) => (
            <button type="button" key={person.id} className="pixel" onClick={() => onMember(person)}>
              <div className="pixelPerson tone-d" />
              <small>{person.displayName} · 在线</small>
            </button>
          ))}
        </div>
      </div>
      <p className="small muted" style={{ marginTop: 18 }}>↗ 点击成员查看详情。场景为只读投影，不直接触发任务。</p>
    </div>
  );
}

/* ---------------------------------------------------------------- 扩展 */

export function ExtensionsPage(props: PreviewPageProps) {
  const { group, onRefresh } = props;
  const [extensions, setExtensions] = useState<ExtensionStatus[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    if (!group) return;
    try {
      setExtensions(await api.listGroupExtensions(group.id));
      setError(null);
    } catch (reason) {
      setError(errText(reason));
    }
  }, [group?.id]);

  useEffect(() => { void load(); }, [load]);

  return (
    <div className="pageScroll">
      <Heading over="EXTENSIONS" title="连接更多可能" sub="扩展按群启用，始终保持在工作台上下文中。" />
      {error && <PageError message={error} />}
      {extensions.map((ext) => (
        <div className="panel" key={ext.id}>
          <div className="panelHead">
            <h3>◉ {ext.name} <span className="tag">{ext.version}</span></h3>
            <label className="pillSwitch">
              <input
                type="checkbox"
                checked={ext.enabled}
                disabled={busy}
                onChange={(e) => {
                  setBusy(true);
                  void api.setExtensionEnabled(group?.id ?? "", ext.id, e.target.checked)
                    .then(() => { void load(); void onRefresh(); })
                    .catch((reason) => setError(errText(reason)))
                    .finally(() => setBusy(false));
                }}
              />启用
            </label>
          </div>
          <p className="small muted">{ext.healthDetail || "扩展运行空间。"} {ext.healthy ? "" : "（当前不可用）"}</p>
          <button type="button" className="secondary" disabled={!ext.enabled}>打开扩展 ↗</button>
        </div>
      ))}
      {extensions.length === 0 && <div className="panel"><p className="small muted">当前群还没有可用扩展。</p></div>}
    </div>
  );
}

/* ---------------------------------------------------------------- Agent 环境 */

const ADAPTER_LABEL: Record<string, string> = {
  "claude-code": "claude-code", dsh: "dsh", opencode: "opencode", codex: "codex", cursor: "cursor", mock: "mock", custom: "custom", chatbot: "chatbot",
};

export function AgentsPage(props: PreviewPageProps) {
  const { members, onMember, onRefresh } = props;
  const agents = members.filter((m) => m.kind === "agent");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const detectAll = async () => {
    setBusy(true);
    try {
      for (const agent of agents) await api.detectAgent(agent.id).catch(() => undefined);
      await onRefresh();
    } catch (reason) {
      setError(errText(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="pageScroll">
      <Heading
        over="AGENT ENVIRONMENT"
        title="你的 Agent 同事"
        sub="适配器、模型与工作环境，在一处管理。"
        action={<button type="button" className="primary" disabled={busy} onClick={() => void detectAll()}>↻ 检测环境</button>}
      />
      {error && <PageError message={error} />}
      <div className="configGrid">
        {agents.map((agent) => (
          <div className="panel" key={agent.id}>
            <div className="panelHead">
              <div className={`avatar ${agent.runtimeStatus === "ready" ? "b" : ""}`}>{agent.displayName.slice(0, 1)}</div>
              <span className={`tag ${agent.runtimeStatus === "ready" ? "green" : "amber"}`}>{agent.runtimeStatus === "ready" ? "就绪" : agent.runtimeStatus === "unavailable" ? "不可用" : "待检测"}</span>
            </div>
            <h3 style={{ margin: 0 }}>{agent.displayName} <span className="tag">{ADAPTER_LABEL[agent.adapter ?? ""] ?? agent.adapter ?? "未配置"}</span></h3>
            <p className="muted small">{agent.roleDescription || "—"}<br />{agent.model || "默认模型"}</p>
            <button type="button" className="secondary" onClick={() => onMember(agent)}>配置与详情 ↗</button>
          </div>
        ))}
        {agents.length === 0 && <div className="panel"><p className="small muted">还没有 Agent 成员。</p></div>}
      </div>
    </div>
  );
}

/* ---------------------------------------------------------------- 可观测 */

export function LogsPage({ runs }: PreviewPageProps) {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [filter, setFilter] = useState("所有级别");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setLogs(await api.listLogs({ limit: 40 }));
      setError(null);
    } catch (reason) {
      setError(errText(reason));
    }
  }, []);

  useEffect(() => { void load(); }, [load]);

  const active = runs.filter((run) => run.status === "running" || run.status === "queued").length;
  const errors = logs.filter((log) => log.level === "error").length;
  const shown = logs.filter((log) => filter === "所有级别" || log.level.toUpperCase() === filter);

  return (
    <div className="pageScroll">
      <Heading
        over="OBSERVABILITY"
        title="运行的每一步，都可追溯"
        sub="日志、经验与运维状态，帮助你从结果回到过程。"
        action={<button type="button" className="secondary" disabled={busy} onClick={() => { setBusy(true); void load().finally(() => setBusy(false)); }}>↻ 刷新日志</button>}
      />
      <Stats items={[["服务状态", "正常", "运行中"], ["活动任务", String(active), "项"], ["最新错误", String(errors), "条"]]} />
      {error && <PageError message={error} />}
      <div className="panel">
        <div className="panelHead">
          <h3>最近日志</h3>
          <select style={{ width: 110, fontSize: 10, padding: 5 }} value={filter} onChange={(e) => setFilter(e.target.value)}>
            <option>所有级别</option><option>INFO</option><option>WARN</option><option>ERROR</option>
          </select>
        </div>
        {shown.map((log) => (
          <div className="log" key={log.id}>
            <span style={{ color: "var(--faint)" }}>{new Date(log.createdAt).toLocaleTimeString("zh-CN", { hour12: false })}</span>{" "}
            <span style={{ color: log.level === "error" ? "var(--red)" : log.level === "warn" ? "var(--amber)" : "var(--green)" }}>{log.level.toUpperCase()}</span>{" "}
            <span>{log.source}</span><br />
            <span style={{ color: "var(--dim)", paddingLeft: 64 }}>{log.message}{log.details ? ` · ${log.details}` : ""}</span>
          </div>
        ))}
        {shown.length === 0 && <p className="small muted">没有匹配的日志。</p>}
      </div>
    </div>
  );
}

/* ---------------------------------------------------------------- 设置 */

export function SettingsPage(props: PreviewPageProps) {
  const { light, onToggleLight } = props;
  const [settings, setSettings] = useState<RuntimeSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    void api.getSettings().then(setSettings).catch((reason) => setError(errText(reason)));
  }, []);

  return (
    <div className="pageScroll">
      <Heading over="PREFERENCES" title="让工作台适合你" sub="外观与输入偏好，不打断正在进行的协作。" />
      {error && <PageError message={error} />}
      <div className="panel">
        <div className="panelHead"><h3>外观主题</h3><span className="tag">设计 1105</span></div>
        <div className="buttonRow">
          <button type="button" className={light ? "secondary" : "primary"} onClick={() => { if (light) onToggleLight(); }}>● Graphite · 深色</button>
          <button type="button" className={light ? "primary" : "secondary"} onClick={() => { if (!light) onToggleLight(); }}>○ Atlas · 浅色</button>
        </div>
        <p className="muted small">当前发布保留深浅两种视觉方向；旧七套主题在旧壳中仍可用。</p>
      </div>
      <div className="panel">
        <h3>运行时偏好</h3>
        <label className="formLabel">心跳间隔（秒）
          <input
            type="number"
            min={1}
            value={settings?.heartbeatFocusSeconds ?? 15}
            onChange={(e) => setSettings((prev) => prev ? { ...prev, heartbeatFocusSeconds: Number(e.target.value) || 1 } : prev)}
          />
        </label>
        <label className="pillSwitch"><input type="checkbox" checked={settings?.heartbeatAuto ?? true} onChange={(e) => setSettings((prev) => prev ? { ...prev, heartbeatAuto: e.target.checked } : prev)} />自动心跳</label>
        <div className="buttonRow" style={{ marginTop: 20 }}>
          <button type="button" className="primary" disabled={!settings} onClick={() => {
            if (!settings) return;
            setSaved(false);
            void api.updateSettings(settings).then((next) => { setSettings(next); setSaved(true); }).catch((reason) => setError(errText(reason)));
          }}>保存设置</button>
          {saved && <span className="small muted">已保存</span>}
        </div>
      </div>
      <div className="panel">
        <h3>关于此设计</h3>
        <p className="small muted">UI / UX Design 01 · 2026.11<br />群即工作空间 · Agent 是同事 · 人在关键节点拍板</p>
      </div>
    </div>
  );
}
