//! 参考站 ohmyworkpanel-design-1105 的 1:1 前台实现（第一阶段：主壳 + 协作）。
//!
//! 类名/结构/尺寸严格对齐参考站；数据来自真实 group state。
//! 入口：`?design=1105` 或 localStorage `omwp.design=1105`。

import { useEffect, useMemo, useRef, useState } from "react";

import { partsToPlainText, projectContentForList } from "../chat/messageContent";
import type { GoalBarState } from "../hooks/useGoalBar";
import type { Group, GroupState, Member, Message, TaskRun } from "../types";
import "./preview.css";

const TABS = [
  ["chat", "◷", "协作"],
  ["versions", "▤", "交付"],
  ["goals", "◎", "目标"],
  ["marketing", "◇", "内容"],
  ["scene", "▦", "场景"],
  ["extensions", "⊞", "扩展"],
] as const;

const CHAT_GROUPS_HIDE = new Set(["versions", "goals", "marketing"]);

const AVATAR_TONES = ["", "a", "b", "c"];

export type PreviewWorkbenchProps = {
  groups: Group[];
  archived: Group[];
  current: GroupState | null;
  onlineUserIds: ReadonlySet<string> | null;
  connected: boolean;
  view: string;
  goalBar: GoalBarState | null;
  senderMemberId: string | null;
  isAdmin: boolean;
  onSelectGroup: (group: Group) => void;
  onCreateGroup: () => void;
  onView: (view: string) => void;
  onSend: (text: string) => Promise<void> | void;
  onMember: (member: Member) => void;
  onReview: (run: TaskRun, decision: "approved" | "rejected") => void;
  onCancelRun: (run: TaskRun) => void;
  onOpenSettings: () => void;
};

function timeLabel(ts: number): string {
  return new Intl.DateTimeFormat("zh-CN", { hour: "2-digit", minute: "2-digit" }).format(ts);
}

function dayLabel(ts: number): string {
  const d = new Date(ts);
  return `${d.getMonth() + 1} 月 ${d.getDate()} 日`;
}

function roleLabel(member: Member | undefined): string {
  if (!member) return "";
  if (member.roleDescription === "owner") return "群主";
  return member.roleDescription || member.adapter || "";
}

function renderContent(content: string, members: Member[]) {
  const names = members.map((m) => m.displayName).filter(Boolean);
  if (names.length === 0) return content;
  const pattern = new RegExp(`@(${names.map((n) => n.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")})`, "g");
  const parts = content.split(pattern);
  const out: React.ReactNode[] = [];
  let buffer = "";
  for (let i = 0; i < parts.length; i += 1) {
    if (i % 2 === 0) {
      buffer += parts[i];
    } else {
      if (buffer) out.push(buffer);
      buffer = "";
      out.push(
        <span className="mention" key={`m-${i}`}>
          @{parts[i]}
        </span>,
      );
    }
  }
  if (buffer) out.push(buffer);
  return out.length ? out : content;
}

export function PreviewWorkbench(props: PreviewWorkbenchProps) {
  const {
    groups,
    current,
    onlineUserIds,
    connected,
    view,
    goalBar,
    senderMemberId,
    onSelectGroup,
    onCreateGroup,
    onView,
    onSend,
    onMember,
    onReview,
    onCancelRun,
    onOpenSettings,
  } = props;

  const [light, setLight] = useState(false);
  const [phone, setPhone] = useState(false);
  const [rightTab, setRightTab] = useState<"members" | "queue" | "details">("members");
  const [search, setSearch] = useState("");
  const [showGuide, setShowGuide] = useState(false);
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const [memberOpen, setMemberOpen] = useState(false);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [noRight, setNoRight] = useState(false);
  const transcriptRef = useRef<HTMLDivElement | null>(null);

  const group = current?.group ?? null;
  const members = current?.members ?? [];
  const messages = current?.messages ?? [];
  const runs = current?.runs ?? [];
  const isChatGroup = group?.groupKind === "chat";
  const agents = members.filter((m) => m.kind === "agent");
  const people = members.filter((m) => m.kind !== "agent");

  const myMessages = useMemo(
    () => messages.filter((m) => m.status !== "streaming" || m.senderMemberId === senderMemberId),
    [messages, senderMemberId],
  );

  useEffect(() => {
    const node = transcriptRef.current;
    if (node) node.scrollTop = node.scrollHeight;
  }, [current?.group.id, messages.length]);

  const filteredGroups = groups.filter((g) => g.name.toLowerCase().includes(search.toLowerCase()));

  const send = async () => {
    const text = draft.trim();
    if (!text || sending) return;
    setSending(true);
    try {
      await onSend(text);
      setDraft("");
    } finally {
      setSending(false);
    }
  };

  const toggleRight = () => {
    if (phone || window.innerWidth <= 1020) {
      setMemberOpen((open) => !open);
    } else {
      setNoRight((open) => !open);
    }
  };

  const runFor = (message: Message) => runs.find((run) => run.outputMessageId === message.id);
  const activeRuns = runs.filter((run) => run.status === "running" || run.status === "queued");

  return (
    <div className={`pv-root${light ? " light" : ""}`}>
      <div className="review">
        <span>
          <b>DESIGN PREVIEW</b>
          <i />
          ohMyWorkPanel / UI &amp; UX 01
        </span>
        <div>
          <span className="sample">演示数据 · 操作仅在此页面生效</span>
          <button type="button" id="mobilePreview" onClick={() => setPhone((v) => !v)}>
            {phone ? "▣" : "▯"} <span>{phone ? "桌面预览" : "手机预览"}</span>
          </button>
          <button type="button" id="theme" onClick={() => setLight((v) => !v)}>
            ◐ <span>切换主题</span>
          </button>
          <button type="button" id="guideBtn" onClick={() => setShowGuide(true)}>设计说明 ↗</button>
        </div>
      </div>

      <div className={`app${phone ? " phone" : ""}${noRight ? " noRight" : ""}`} id="app">
        <nav className="rail">
          <div className="logo">w<span>•</span></div>
          <button type="button" className={`railItem${view === "chat" ? " selected" : ""}`} title="工作台" onClick={() => onView("chat")}>▦</button>
          <button type="button" className={`railItem${view === "agent-config" ? " selected" : ""}`} title="Agent 环境" onClick={() => onView("agent-config")}>♧</button>
          <button type="button" className={`railItem${view === "observability" ? " selected" : ""}`} title="可观测" onClick={() => onView("observability")}>▥</button>
          <div className="railBottom">
            <button type="button" className={`railItem${view === "settings" ? " selected" : ""}`} title="设置" onClick={onOpenSettings}>⚙</button>
            <div className="avatar you">L</div>
          </div>
        </nav>

        <aside className={`sidebar${sidebarOpen ? " open" : ""}`}>
          <div className="brand">ohMyWorkPanel<span>LOCAL WORKSPACE</span></div>
          <div className="sideHeading">
            工作空间 <button type="button" title="创建群" onClick={onCreateGroup}>＋</button>
          </div>
          <label className="search">
            ⌕ <input placeholder="查找群组" value={search} onChange={(e) => setSearch(e.target.value)} />
            <kbd>⌘ K</kbd>
          </label>
          <div className="groupList">
            {filteredGroups.map((g) => {
              const active = g.id === group?.id;
              return (
                <button type="button" key={g.id} className={`group${active ? " active" : ""}`} onClick={() => { onSelectGroup(g); setSidebarOpen(false); }}>
                  <span className="groupIcon">{g.groupKind === "chat" ? "☕" : "◈"}</span>
                  <span>{g.name}<small>{g.groupKind === "chat" ? "人与 Agent 的轻松交流" : "多 Agent 协作工作台"}</small></span>
                  {active && <b />}
                  {(g.unreadCount ?? 0) > 0 && <em>{g.unreadCount}</em>}
                </button>
              );
            })}
          </div>
          <div className="sideHeading sub">工作台</div>
          <button type="button" className="sideLink" onClick={() => onView("agent-config")}>♧ Agent 环境 <em>{agents.length}</em></button>
          <button type="button" className="sideLink" onClick={() => onView("observability")}>▥ 日志与经验</button>
          <div className="sidebarFooter">
            <div className="server"><span className="dot" /> 本地服务运行中 <small>v2.1.2</small></div>
            <div className="profile">
              <div className="avatar you">L</div>
              <div>Ling Li<small>平台管理员</small></div>
              <button type="button" onClick={onOpenSettings}>⋯</button>
            </div>
          </div>
        </aside>

        <main>
          <header className="header">
            <button type="button" className="icon mobileOnly" title="打开群列表" onClick={() => setSidebarOpen((v) => !v)}>☰</button>
            <div className="projectIcon">◈</div>
            <div>
              <h1>{group?.name ?? "未选择群"} <span>{isChatGroup ? "聊天群" : "项目群"}</span></h1>
              <p>⌁ {group?.workspacePath ?? "/workspace"}</p>
            </div>
            <div className="headRight">
              <span className="connected"><span className="dot" /> {connected ? "已连接" : "连接中"}</span>
              <button type="button" className="icon" title="群公告">⚑</button>
              <button type="button" className="membersBtn" onClick={toggleRight}>
                {agents.slice(0, 3).map((a, index) => (
                  <span key={a.id} className={`tinyAv ${AVATAR_TONES[index % 4]}`}>{a.displayName.slice(0, 1)}</span>
                ))}
                <span>{members.length} 位成员</span>
              </button>
              <button type="button" className="icon" title="群设置" onClick={onOpenSettings}>⋯</button>
            </div>
          </header>

          <div className="tabs" id="tabs">
            {TABS.filter(([id]) => !(isChatGroup && CHAT_GROUPS_HIDE.has(id))).map(([id, icon, label]) => (
              <button type="button" key={id} className={view === id ? "active" : ""} onClick={() => onView(id)}>
                <span>{icon}</span>{label}
              </button>
            ))}
          </div>

          <div id="content">
            {view === "chat" ? (
              <>
                {!isChatGroup && (
                  <div className="summary">
                    <span style={{ color: "var(--accent)" }}>◉</span>
                    <strong>{goalBar?.versionName || "v2.1.2 · 当前工作台"}</strong>
                    <span className="tag green">{goalBar?.waveTitle || "尚无 Wave"}</span>
                    <div className="progress"><i style={{ width: `${goalBar?.total ? Math.round((goalBar.done / goalBar.total) * 100) : 0}%` }} /></div>
                    <button type="button" onClick={() => onView("versions")}>查看交付 →</button>
                  </div>
                )}
                <div className="transcript" ref={transcriptRef}>
                  {messages.length > 0 && <div className="dateLine">今天 · {dayLabel(messages[messages.length - 1].createdAt)}</div>}
                  {myMessages.map((message) => {
                    const sender = members.find((m) => m.id === message.senderMemberId);
                    const run = runFor(message);
                    const own = message.senderMemberId === senderMemberId;
                    const streaming = message.status === "streaming" || run?.status === "running" || run?.status === "queued";
                    const avatarTone = own ? "you" : AVATAR_TONES[(members.findIndex((m) => m.id === message.senderMemberId) % 3) + 1] || "a";
                    const projected = projectContentForList(message.content);
                    const displayText = partsToPlainText(projected.content);
                    return (
                      <article key={message.id} className={`message${own ? " user" : ""}`}>
                        <div className={`avatar ${avatarTone}`}>{sender?.displayName.slice(0, 1) ?? "?"}</div>
                        <div className="messageBody">
                          <div className="messageMeta">
                            <strong>{sender?.displayName ?? "已移除成员"}</strong>
                            <small>{roleLabel(sender)}</small>
                            {sender?.adapter && <span className="tag">{sender.adapter}</span>}
                            <time>{timeLabel(message.createdAt)}</time>
                          </div>
                          <div className="bubble">
                            {message.hasThinking && (
                              <button type="button" className="disclosure" title="执行摘要">
                                ▸ 分析与执行过程
                                <span style={{ float: "right", color: "var(--faint)" }}>
                                  {streaming ? "进行中…" : "已完成"}
                                </span>
                              </button>
                            )}
                            <p>{renderContent(displayText || (streaming ? "…" : ""), members)}</p>
                            {streaming && (
                              <div className="runLine">
                                <span className="dot" />
                                {run?.status === "queued" ? "排队中" : "运行中"}
                                {run?.phase && <span>· {run.phase}</span>}
                                <div className="track"><i /></div>
                                {run && (
                                  <button type="button" style={{ color: "var(--dim)", fontSize: 9, marginLeft: "auto" }} onClick={() => onCancelRun(run)}>
                                    查看详情 ↗
                                  </button>
                                )}
                              </div>
                            )}
                            {!streaming && run && (
                              <div className="runLine">
                                <span className="dot" /> 执行完成
                                <span>·</span> {timeLabel(run.completedAt ?? message.createdAt)}
                                <span style={{ marginLeft: "auto" }}>
                                  {run.reviewStatus === "approved" ? "✓ 已审阅" : run.reviewStatus === "pending" ? "等待你的审阅" : (run.status === "failed" ? "失败" : "已完成")}
                                </span>
                              </div>
                            )}
                          </div>
                          {!own && !streaming && (
                            <div className="messageTools">
                              <button type="button" onClick={() => { if (run) onReview(run, "approved"); }}>✓ 通过审阅</button>
                              <button type="button" onClick={() => { if (run) onReview(run, "rejected"); }}>↺ 要求修改</button>
                              <button type="button">↩ 引用</button>
                              <button type="button" onClick={() => void navigator.clipboard?.writeText(displayText)}>▢ 复制</button>
                            </div>
                          )}
                        </div>
                      </article>
                    );
                  })}
                  {messages.length === 0 && (
                    <div className="empty">
                      <h3>{isChatGroup ? "今天有什么新想法？" : "发布第一个任务"}</h3>
                      <p>{isChatGroup ? "在这里和你的 Agent 同事轻松聊聊。" : "在下方 @ 一位 Agent，开始协作。"}</p>
                    </div>
                  )}
                </div>
                <div className="composerWrap">
                  <div className="composer">
                    <textarea
                      placeholder="发布一个任务，@ 你的 Agent 同事…"
                      aria-label="消息内容"
                      value={draft}
                      onChange={(e) => setDraft(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                          e.preventDefault();
                          void send();
                        }
                      }}
                    />
                    <div className="composerTools">
                      <button type="button" title="提及成员">＠</button>
                      <button type="button" title="命令菜单">/</button>
                      <button type="button" title="OCR">▧</button>
                      <span style={{ height: 12, width: 1, background: "var(--border)" }} />
                      <small>默认响应：{agents.find((a) => a.id === group?.adminMemberId)?.displayName ?? "未设置"}</small>
                      <button type="button" className="send" onClick={() => void send()} disabled={sending || !draft.trim()}>发送 ↑</button>
                    </div>
                  </div>
                  <div className="composerFoot">
                    <span>Enter 发送 · Shift + Enter 换行</span>
                    <span>⌁ 草稿仅保存在本次会话</span>
                  </div>
                </div>
              </>
            ) : (
              <div className="pageScroll">
                <div className="pageHeading">
                  <div>
                    <div className="overline">WORKBENCH / MIGRATING</div>
                    <h2>该页正在按参考站迁移</h2>
                    <p>主壳与协作页已 1:1 对齐；此页将在下一阶段像素级实现。</p>
                  </div>
                  <button type="button" className="secondary" onClick={() => onView("chat")}>返回协作 ↗</button>
                </div>
              </div>
            )}
          </div>
        </main>

        <aside className={`inspector${memberOpen ? " open" : ""}`}>
          <div className="rightTabs">
            <button type="button" className={rightTab === "members" ? "active" : ""} onClick={() => setRightTab("members")}>成员<small>{members.length}</small></button>
            <button type="button" className={rightTab === "queue" ? "active" : ""} onClick={() => setRightTab("queue")}>队列<small>{activeRuns.length}</small></button>
            <button type="button" className={rightTab === "details" ? "active" : ""} onClick={() => setRightTab("details")}>详情</button>
          </div>
          <div id="rightContent">
            {rightTab === "members" && (
              <>
                <div className="eyebrow">AGENTS <span>{agents.length} 位</span></div>
                {agents.map((agent, index) => {
                  const active = activeRuns.find((run) => run.agentMemberId === agent.id);
                  return (
                    <button type="button" key={agent.id} className="memberRow" onClick={() => onMember(agent)}>
                      <div className={`avatar ${AVATAR_TONES[(index % 3) + 1]}`}>{agent.displayName.slice(0, 1)}</div>
                      <div className="memberInfo">
                        {agent.displayName}
                        {agent.id === group?.adminMemberId && <span className="tag">管理员</span>}
                        <small>{roleLabel(agent)}</small>
                        <div className={`memberStatus${active ? "" : " idle"}`}>
                          {active ? (active.status === "queued" ? "排队中" : "正在执行任务") : "空闲 · 随时待命"}
                        </div>
                      </div>
                    </button>
                  );
                })}
                <button type="button" className="addMember" onClick={onCreateGroup}>＋ 添加 Agent 或成员</button>
                <div className="eyebrow">PEOPLE <span>{people.length} 位</span></div>
                {people.map((person, index) => {
                  const online = Boolean(person.authUserId && onlineUserIds?.has(person.authUserId));
                  return (
                    <button type="button" key={person.id} className="memberRow" onClick={() => onMember(person)}>
                      <div className={`avatar ${index === 0 ? "you" : ""}`}>{person.displayName.slice(0, 1)}</div>
                      <div className="memberInfo">
                        {person.displayName}
                        <small>{roleLabel(person)}{online ? " · 在线" : " · 离线"}</small>
                      </div>
                    </button>
                  );
                })}
                <div className="rightCard">
                  <div className="eyebrow" style={{ marginBottom: 10 }}>CURRENT GOAL <span>↗</span></div>
                  <h4>{goalBar?.waveTitle || "尚未建立目标"}</h4>
                  <p>{goalBar ? "Agent 正在自主推进" : "创建 Goal 后，Leader Agent 会按预算推进。"}</p>
                  <div className="progress"><i style={{ width: `${goalBar?.total ? Math.round((goalBar.done / goalBar.total) * 100) : 0}%` }} /></div>
                  <footer>
                    <span>{goalBar ? `${goalBar.done} / ${goalBar.total} 阶段` : "—"}</span>
                    <button type="button" style={{ fontSize: 9, color: "var(--accent)", padding: 0 }} onClick={() => onView("goals")}>查看证据 →</button>
                  </footer>
                </div>
              </>
            )}
            {rightTab === "queue" && (
              <>
                <div className="eyebrow">CURRENT RUNS <span>当前群</span></div>
                {activeRuns.length === 0 && <div className="empty">当前没有排队或运行中的任务</div>}
                {activeRuns.map((run) => {
                  const agent = members.find((m) => m.id === run.agentMemberId);
                  return (
                    <div key={run.id} className="rightCard" style={{ margin: "0 0 13px" }}>
                      <div className="panelHead" style={{ marginBottom: 8 }}>
                        <h4>{agent?.displayName ?? "Agent"}</h4>
                        <span className={`tag ${run.status === "running" ? "blue" : ""}`}>{run.status === "running" ? "运行中" : "排队中"}</span>
                      </div>
                      <p>{run.phase || "等待执行"}</p>
                      <button type="button" className="secondary" style={{ marginTop: 13 }} onClick={() => onCancelRun(run)}>执行详情 ↗</button>
                    </div>
                  );
                })}
              </>
            )}
            {rightTab === "details" && (
              <>
                <div className="eyebrow">CONTEXT / DETAILS</div>
                <div className="rightCard">
                  <h4>{group?.name ?? "未选择群"}</h4>
                  <p>选中一条消息、任务或成员，在此查看关联信息。</p>
                </div>
                <div className="rightCard">
                  <h4>项目公告</h4>
                  <p>{group?.announcement?.trim() || "群公告尚未设置。"}</p>
                </div>
              </>
            )}
          </div>
          <div className="rightFoot"><span className="dot" /> 所有 Agent 状态已同步 <span>v2.1.2</span></div>
        </aside>
      </div>

      {showGuide && (
        <dialog open>
          <div className="modalTop"><h2>设计说明 / Workbench 01</h2><button type="button" onClick={() => setShowGuide(false)}>×</button></div>
          <div className="overline">CALM SPACE. CLEAR PROGRESS.</div>
          <p>群是稳定的协作空间；中心区域服务当前任务，上下文在右侧渐进展开。</p>
          <div className="designList">
            <div><strong>01 · 群是稳定的协作空间</strong><small>项目、成员、版本和目标共用同一上下文。</small></div>
            <div><strong>02 · 看得见谁在做什么</strong><small>运行状态、角色与队列直接可见。</small></div>
            <div><strong>03 · 人在关键节点拍板</strong><small>Goal 轮次、证据与预算并列。</small></div>
            <div><strong>04 · 渐进迁移</strong><small>保留 React、语义 token、双端 API 与插件边界。</small></div>
          </div>
          <button type="button" className="primary" onClick={() => setShowGuide(false)}>知道了</button>
        </dialog>
      )}
    </div>
  );
}
