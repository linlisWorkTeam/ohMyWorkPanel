import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import { boundWorkForItem, pickDefaultAssigneeId, seedChecklistTitles } from "./roadmapUi";
import type {
  Member, Feature, FeatureTask, RoadmapItem, RoadmapState,
  CreateRoadmapItemInput, UpdateRoadmapItemInput,
  CreateFeatureInput, UpdateFeatureInput,
} from "./types";

// ── helpers ──

const STATUS_LABEL: Record<string, string> = {
  backlog: "待办", in_progress: "进行中", review: "评审", done: "完成",
};
const PRIORITY_LABEL: Record<string, string> = {
  low: "低", medium: "中", high: "高", critical: "紧急",
};
const PRIORITY_COLOR: Record<string, string> = {
  low: "#6c819c", medium: "#cf8a2c", high: "#c65a3b", critical: "#c13838",
};

function timeAgo(ts: number) {
  // Backend timestamps are milliseconds since epoch (db.now()).
  const millis = ts < 1e12 ? ts * 1000 : ts;
  const min = Math.floor((Date.now() - millis) / 60000);
  if (min < 1) return "刚刚";
  if (min < 60) return `${min}分钟前`;
  const h = Math.floor(min / 60);
  if (h < 24) return `${h}小时前`;
  return `${Math.floor(h / 24)}天前`;
}

function id() { return crypto.randomUUID(); }

// ── main panel ──

interface PmPanelProps {
  groupId: string;
  members: Member[];
  adminMemberId?: string | null;
  onError: (msg: string) => void;
}

export function PmPanel({ groupId, members, adminMemberId, onError }: PmPanelProps) {
  const [tab, setTab] = useState<"roadmap" | "features">("roadmap");
  const [state, setState] = useState<RoadmapState | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);

  const load = useCallback(async (soft = false) => {
    if (!soft) setLoading(true);
    else setRefreshing(true);
    try {
      const s = await api.getRoadmapState(groupId);
      setState(s);
    } catch (e: unknown) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, [groupId, onError]);

  useEffect(() => { void load(); }, [load]);

  const refreshSoft = () => void load(true);

  if (loading) return <div className="pm-loading">加载中…</div>;
  if (!state) return <div className="pm-loading">无法加载项目管理数据</div>;

  return (
    <div className="pm-panel">
      <nav className="pm-tabs">
        <button className={`pm-tab ${tab === "roadmap" ? "active" : ""}`} onClick={() => setTab("roadmap")}>路线图</button>
        <button className={`pm-tab ${tab === "features" ? "active" : ""}`} onClick={() => setTab("features")}>功能看板</button>
        {refreshing && <span className="pm-refreshing" title="刷新中">同步中…</span>}
      </nav>
      <div className="pm-body">
        {tab === "roadmap" ? (
          <RoadmapView groupId={groupId} items={state.items} features={state.features} tasks={state.tasks} members={members} adminMemberId={adminMemberId} onUpdate={refreshSoft} onError={onError} />
        ) : (
          <FeatureKanban groupId={groupId} features={state.features} tasks={state.tasks} members={members} roadmapItems={state.items} onUpdate={refreshSoft} onError={onError} />
        )}
      </div>
    </div>
  );
}

// ── Roadmap View ──

function RoadmapView({ groupId, items, features, tasks, members, adminMemberId, onUpdate, onError }: {
  groupId: string; items: RoadmapItem[]; features: Feature[]; tasks: FeatureTask[]; members: Member[];
  adminMemberId?: string | null;
  onUpdate: () => void; onError: (m: string) => void;
}) {
  const [showForm, setShowForm] = useState(false);
  const [editing, setEditing] = useState<RoadmapItem | null>(null);
  const [seedingId, setSeedingId] = useState<string | null>(null);

  const statusOrder = ["backlog", "in_progress", "review", "done"];
  const grouped = statusOrder.map((s) => ({ status: s, items: items.filter((i) => i.status === s) }));

  const handleCreate = async (input: CreateRoadmapItemInput) => {
    try { await api.createRoadmapItem(input); setShowForm(false); onUpdate(); } catch (e: unknown) { onError(e instanceof Error ? e.message : String(e)); }
  };
  const handleUpdate = async (id: string, input: UpdateRoadmapItemInput) => {
    try { await api.updateRoadmapItem(id, input); setEditing(null); onUpdate(); } catch (e: unknown) { onError(e instanceof Error ? e.message : String(e)); }
  };
  const handleDelete = async (id: string) => {
    if (!confirm("确定删除此路线图项？")) return;
    try { await api.deleteRoadmapItem(id); onUpdate(); } catch (e: unknown) { onError(e instanceof Error ? e.message : String(e)); }
  };

  const seedChecklist = async (item: RoadmapItem) => {
    const work = boundWorkForItem(item.id, features, tasks);
    if (work.totalTasks > 0) {
      onError("该项已有 checklist，无需生成。");
      return;
    }
    setSeedingId(item.id);
    try {
      const seed = seedChecklistTitles(item.title);
      const assignee = pickDefaultAssigneeId(members, adminMemberId);
      let featureId: string;
      if (work.features.length > 0) {
        featureId = work.features[0].id;
        if (assignee && !work.features[0].assigneeMemberId) {
          await api.updateFeature(featureId, { assigneeMemberId: assignee });
        }
      } else {
        const feature = await api.createFeature({
          groupId,
          title: seed.featureTitle,
          status: "backlog",
          priority: item.priority || "medium",
          assigneeMemberId: assignee,
          targetRoadmapItemId: item.id,
        });
        featureId = feature.id;
      }
      for (const title of seed.tasks) {
        await api.createFeatureTask({ featureId, title });
      }
      onUpdate();
    } catch (e: unknown) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setSeedingId(null);
    }
  };

  if (items.length === 0 && !showForm) {
    return (
      <div className="pm-empty">
        <p>还没有路线图项</p>
        <p className="pm-empty-hint">创建后到「功能看板」把功能绑定到该项并添加 checklist，即可在上方 Roadmap 条一键启动 Agent。</p>
        <button className="pm-btn primary" onClick={() => setShowForm(true)}>＋ 创建路线图项</button>
      </div>
    );
  }

  return (
    <div className="roadmap-view">
      <div className="pm-section-header">
        <span className="pm-count">共 {items.length} 项</span>
        <button className="pm-btn primary sm" onClick={() => setShowForm(true)}>＋ 新建</button>
      </div>
      {showForm && <RoadmapForm groupId={groupId} onSubmit={handleCreate} onCancel={() => setShowForm(false)} />}
      {grouped.map((g) => g.items.length > 0 && (
        <div key={g.status} className="roadmap-group">
          <h4 className="roadmap-group-title">{STATUS_LABEL[g.status] || g.status}</h4>
          {g.items.map((item) => {
            const work = boundWorkForItem(item.id, features, tasks);
            return editing?.id === item.id ? (
              <RoadmapForm key={item.id} groupId={groupId} initial={item} onSubmit={(input) => handleUpdate(item.id, input)} onCancel={() => setEditing(null)} />
            ) : (
              <div key={item.id} className="roadmap-card">
                <div className="roadmap-card-header">
                  <strong>{item.title}</strong>
                  <span className="pm-badge" style={{ color: PRIORITY_COLOR[item.priority] || "#6c819c", borderColor: PRIORITY_COLOR[item.priority] || "#6c819c" }}>
                    {PRIORITY_LABEL[item.priority] || item.priority}
                  </span>
                </div>
                {item.description && <p className="roadmap-card-desc">{item.description}</p>}
                <div className="roadmap-card-meta">
                  <span className={work.totalTasks === 0 ? "pm-bind warn" : "pm-bind ok"}>
                    {work.features.length === 0
                      ? "未绑定功能"
                      : work.totalTasks === 0
                        ? `${work.features.length} 功能 · 无 checklist`
                        : `${work.features.length} 功能 · checklist ${work.doneTasks}/${work.totalTasks}`}
                  </span>
                  {item.targetDate && <span>📅 {item.targetDate}</span>}
                  <span className="pm-muted">{timeAgo(item.createdAt)}</span>
                </div>
                <div className="roadmap-card-actions">
                  {work.totalTasks === 0 && (
                    <button
                      className="pm-btn primary sm"
                      disabled={seedingId === item.id}
                      onClick={() => void seedChecklist(item)}
                      title="创建绑定功能 + 3 条 checklist，并尽量指派 Agent"
                    >
                      {seedingId === item.id ? "生成中…" : "一键 checklist"}
                    </button>
                  )}
                  <button className="pm-link" onClick={() => setEditing(item)}>编辑</button>
                  <button className="pm-link danger" onClick={() => handleDelete(item.id)}>删除</button>
                </div>
              </div>
            );
          })}
        </div>
      ))}
    </div>
  );
}

function RoadmapForm({ groupId, initial, onSubmit, onCancel }: {
  groupId: string; initial?: RoadmapItem; onSubmit: (input: CreateRoadmapItemInput) => void; onCancel: () => void;
}) {
  const [title, setTitle] = useState(initial?.title ?? "");
  const [desc, setDesc] = useState(initial?.description ?? "");
  const [status, setStatus] = useState(initial?.status ?? "backlog");
  const [priority, setPriority] = useState(initial?.priority ?? "medium");
  const [targetDate, setTargetDate] = useState(initial?.targetDate ?? "");
  return (
    <div className="pm-inline-form">
      <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="标题" required className="pm-input" />
      <textarea value={desc} onChange={(e) => setDesc(e.target.value)} placeholder="描述（可选）" className="pm-textarea" rows={2} />
      <div className="pm-inline-row">
        <select value={status} onChange={(e) => setStatus(e.target.value)} className="pm-select"><option value="backlog">待办</option><option value="in_progress">进行中</option><option value="review">评审</option><option value="done">完成</option></select>
        <select value={priority} onChange={(e) => setPriority(e.target.value)} className="pm-select"><option value="low">低</option><option value="medium">中</option><option value="high">高</option><option value="critical">紧急</option></select>
        <input type="date" value={targetDate} onChange={(e) => setTargetDate(e.target.value)} className="pm-input sm" />
      </div>
      <div className="pm-inline-actions">
        <button className="pm-btn quiet sm" onClick={onCancel}>取消</button>
        <button className="pm-btn primary sm" disabled={!title.trim()} onClick={() => onSubmit({ groupId, title: title.trim(), description: desc || undefined, status, priority, targetDate: targetDate || undefined })}>
          {initial ? "保存" : "创建"}
        </button>
      </div>
    </div>
  );
}

// ── Feature Kanban ──

const STATUSES = ["backlog", "in_progress", "review", "done"];

function FeatureKanban({ groupId, features, tasks, members, roadmapItems, onUpdate, onError }: {
  groupId: string; features: Feature[]; tasks: FeatureTask[]; members: Member[]; roadmapItems: RoadmapItem[]; onUpdate: () => void; onError: (m: string) => void;
}) {
  const [showForm, setShowForm] = useState(false);

  const handleCreate = async (input: CreateFeatureInput) => {
    try { await api.createFeature(input); setShowForm(false); onUpdate(); } catch (e: unknown) { onError(e instanceof Error ? e.message : String(e)); }
  };
  const handleUpdateFeature = async (id: string, input: UpdateFeatureInput) => {
    try { await api.updateFeature(id, input); onUpdate(); } catch (e: unknown) { onError(e instanceof Error ? e.message : String(e)); }
  };
  const handleDeleteFeature = async (id: string) => {
    if (!confirm("确定删除此功能？所有任务也会被删除。")) return;
    try { await api.deleteFeature(id); onUpdate(); } catch (e: unknown) { onError(e instanceof Error ? e.message : String(e)); }
  };

  return (
    <div className="kanban-view">
      <div className="pm-section-header">
        <span className="pm-count">共 {features.length} 个功能</span>
        <button className="pm-btn primary sm" onClick={() => setShowForm(true)}>＋ 新建功能</button>
      </div>
      <p className="pm-flow-hint">编排路径：绑定路线图 → 指派 Agent（或设群管理员 Agent）→ 添加 checklist → 在上方 Roadmap 条点「启动」。</p>
      {showForm && <FeatureForm groupId={groupId} members={members} roadmapItems={roadmapItems} onSubmit={handleCreate} onCancel={() => setShowForm(false)} />}
      <div className="kanban-board">
        {STATUSES.map((status) => (
          <div
            key={status}
            className="kanban-col"
            onDragOver={(e) => { e.preventDefault(); e.dataTransfer.dropEffect = "move"; }}
            onDrop={(e) => {
              e.preventDefault();
              const featureId = e.dataTransfer.getData("text/feature-id");
              const from = e.dataTransfer.getData("text/feature-status");
              if (featureId && from !== status) handleUpdateFeature(featureId, { status });
            }}
          >
            <h4 className="kanban-col-title">{STATUS_LABEL[status] || status}
              <span className="pm-count">{features.filter((f) => f.status === status).length}</span>
            </h4>
            <div className="kanban-cards">
              {features.filter((f) => f.status === status).map((feature) => (
                <FeatureCard
                  key={feature.id}
                  feature={feature}
                  featureTasks={tasks.filter((t) => t.featureId === feature.id)}
                  members={members}
                  roadmapItems={roadmapItems}
                  onUpdate={onUpdate}
                  onError={onError}
                  onDelete={() => handleDeleteFeature(feature.id)}
                  onStatusChange={(s) => handleUpdateFeature(feature.id, { status: s })}
                />
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function FeatureCard({ feature, featureTasks, members, roadmapItems, onUpdate, onError, onDelete, onStatusChange }: {
  feature: Feature; featureTasks: FeatureTask[]; members: Member[]; roadmapItems: RoadmapItem[];
  onUpdate: () => void; onError: (m: string) => void; onDelete: () => void; onStatusChange: (s: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const [editing, setEditing] = useState(false);
  const [newTaskTitle, setNewTaskTitle] = useState("");

  const assignee = members.find((m) => m.id === feature.assigneeMemberId);
  const roadmap = roadmapItems.find((r) => r.id === feature.targetRoadmapItemId);
  const doneCount = featureTasks.filter((t) => t.done).length;
  const bindOk = Boolean(roadmap) && featureTasks.length > 0 && Boolean(assignee);

  const handleAddTask = async () => {
    if (!newTaskTitle.trim()) return;
    try {
      await api.createFeatureTask({ featureId: feature.id, title: newTaskTitle.trim() });
      setNewTaskTitle("");
      onUpdate();
    } catch (e: unknown) { onError(e instanceof Error ? e.message : String(e)); }
  };
  const handleToggleTask = async (task: FeatureTask) => {
    try { await api.updateFeatureTask(task.id, { done: !task.done }); onUpdate(); } catch (e: unknown) { onError(e instanceof Error ? e.message : String(e)); }
  };
  const handleDeleteTask = async (taskId: string) => {
    try { await api.deleteFeatureTask(taskId); onUpdate(); } catch (e: unknown) { onError(e instanceof Error ? e.message : String(e)); }
  };

  return (
    <div
      className={`feature-card ${bindOk ? "ready" : "needs-bind"}`}
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData("text/feature-id", feature.id);
        e.dataTransfer.setData("text/feature-status", feature.status);
        e.dataTransfer.effectAllowed = "move";
      }}
    >
      <div className="feature-card-header" onClick={() => setExpanded(!expanded)}>
        <div className="feature-card-title">
          <strong>{feature.title}</strong>
          <span className="pm-badge" style={{ color: PRIORITY_COLOR[feature.priority] || "#6c819c", borderColor: PRIORITY_COLOR[feature.priority] || "#6c819c" }}>
            {PRIORITY_LABEL[feature.priority] || feature.priority}
          </span>
        </div>
        <div className="feature-card-meta">
          <span className={roadmap ? "pm-bind ok" : "pm-bind warn"}>{roadmap ? `↪ ${roadmap.title}` : "未绑定路线图"}</span>
          {feature.area && <span className="pm-tag">{feature.area}</span>}
          {assignee
            ? <span className="pm-assignee">{assignee.displayName}</span>
            : <span className="pm-bind warn">未指派</span>}
          {featureTasks.length > 0
            ? <span className="pm-tasks-count">{doneCount}/{featureTasks.length}</span>
            : <span className="pm-bind warn">无 checklist</span>}
        </div>
      </div>
      {feature.description && <p className="feature-card-desc">{feature.description}</p>}

      <div className="feature-status-jump">
        {STATUSES.filter((s) => s !== feature.status).map((s) => (
          <button key={s} className="pm-link sm" onClick={() => onStatusChange(s)}>→ {STATUS_LABEL[s]}</button>
        ))}
      </div>

      {expanded && (
        <div className="feature-tasks">
          {featureTasks.map((task) => (
            <div key={task.id} className="feature-task-row">
              <label className="feature-task-label">
                <input type="checkbox" checked={task.done} onChange={() => handleToggleTask(task)} />
                <span className={task.done ? "done" : ""}>{task.title}</span>
              </label>
              <button className="pm-link danger sm" onClick={() => handleDeleteTask(task.id)}>✕</button>
            </div>
          ))}
          <div className="feature-task-add">
            <input
              value={newTaskTitle}
              onChange={(e) => setNewTaskTitle(e.target.value)}
              placeholder="添加子任务…"
              className="pm-input sm"
              onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); void handleAddTask(); } }}
            />
            <button className="pm-btn primary sm" disabled={!newTaskTitle.trim()} onClick={() => void handleAddTask()}>添加</button>
          </div>
        </div>
      )}

      <div className="feature-card-actions">
        <button className="pm-link sm" onClick={() => setEditing(true)}>编辑</button>
        <button className="pm-link danger sm" onClick={onDelete}>删除</button>
      </div>
      {editing && <FeatureForm groupId={feature.groupId} members={members} roadmapItems={roadmapItems} initial={feature} onSubmit={async (input) => {
        try { await api.updateFeature(feature.id, input); setEditing(false); onUpdate(); } catch (e: unknown) { onError(e instanceof Error ? e.message : String(e)); }
      }} onCancel={() => setEditing(false)} />}
    </div>
  );
}

function FeatureForm({ groupId, members, roadmapItems, initial, onSubmit, onCancel }: {
  groupId: string; members: Member[]; roadmapItems: RoadmapItem[]; initial?: Feature; onSubmit: (input: CreateFeatureInput) => void; onCancel: () => void;
}) {
  const [title, setTitle] = useState(initial?.title ?? "");
  const [desc, setDesc] = useState(initial?.description ?? "");
  const [status, setStatus] = useState(initial?.status ?? "backlog");
  const [priority, setPriority] = useState(initial?.priority ?? "medium");
  const [area, setArea] = useState(initial?.area ?? "");
  const [assigneeId, setAssigneeId] = useState(initial?.assigneeMemberId ?? "");
  const [roadmapItemId, setRoadmapItemId] = useState(initial?.targetRoadmapItemId ?? "");
  const agents = members.filter((m) => m.isActive && (m.kind === "agent" || m.kind === "chatbot"));
  return (
    <div className="pm-inline-form">
      <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="功能标题" required className="pm-input" />
      <textarea value={desc} onChange={(e) => setDesc(e.target.value)} placeholder="功能描述（可选）" className="pm-textarea" rows={2} />
      <div className="pm-inline-row">
        <select value={status} onChange={(e) => setStatus(e.target.value)} className="pm-select"><option value="backlog">待办</option><option value="in_progress">进行中</option><option value="review">评审</option><option value="done">完成</option></select>
        <select value={priority} onChange={(e) => setPriority(e.target.value)} className="pm-select"><option value="low">低</option><option value="medium">中</option><option value="high">高</option><option value="critical">紧急</option></select>
      </div>
      <div className="pm-inline-row">
        <input value={area} onChange={(e) => setArea(e.target.value)} placeholder="分类（如 UI/后端）" className="pm-input sm" />
        <select value={assigneeId} onChange={(e) => setAssigneeId(e.target.value)} className="pm-select" title="编排优先使用此 Agent；空则回落群管理员 Agent">
          <option value="">指派 Agent（可空→管理员）</option>
          {agents.map((m) => <option key={m.id} value={m.id}>{m.displayName}</option>)}
        </select>
      </div>
      <div className="pm-inline-row">
        <select value={roadmapItemId} onChange={(e) => setRoadmapItemId(e.target.value)} className="pm-select" disabled={roadmapItems.length === 0}>
          <option value="">{roadmapItems.length === 0 ? "先创建路线图项再绑定" : "绑定到路线图项"}</option>
          {roadmapItems.map((item) => <option key={item.id} value={item.id}>{item.title}</option>)}
        </select>
      </div>
      <div className="pm-inline-actions">
        <button className="pm-btn quiet sm" onClick={onCancel}>取消</button>
        <button className="pm-btn primary sm" disabled={!title.trim()} onClick={() => onSubmit({ groupId, title: title.trim(), description: desc || undefined, status, priority, area: area || undefined, assigneeMemberId: assigneeId || undefined, targetRoadmapItemId: roadmapItemId || undefined })}>
          {initial ? "保存" : "创建"}
        </button>
      </div>
    </div>
  );
}
