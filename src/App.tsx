import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { FormEvent, KeyboardEvent, useEffect, useState, type ReactNode } from "react";
import { api } from "./api";
import { currentMentionQuery, findMentionedMemberIds } from "./mentions";
import type { ChatEvent, Group, GroupState, Member, PresetRole, RuntimeSettings, TaskRun } from "./types";

type NewMember = { kind: "agent" | "user"; displayName: string; roleDescription: string; adapter: string; executablePath: string };
const emptyMember: NewMember = { kind: "agent", displayName: "", roleDescription: "", adapter: "mock", executablePath: "" };
const time = (value: number) => new Intl.DateTimeFormat("zh-CN", { hour: "2-digit", minute: "2-digit" }).format(value);

export function App() {
  const [groups, setGroups] = useState<Group[]>([]);
  const [current, setCurrent] = useState<GroupState | null>(null);
  const [composer, setComposer] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [showMembers, setShowMembers] = useState(true);
  const [showAddMember, setShowAddMember] = useState(false);
  const [newMember, setNewMember] = useState<NewMember>(emptyMember);
  const [ocrRunning, setOcrRunning] = useState(false);
  const [presetRoles, setPresetRoles] = useState<PresetRole[]>([]);
  const [selectedRoles, setSelectedRoles] = useState<string[]>([]);
  const [settings, setSettings] = useState<RuntimeSettings | null>(null);
  const [showSettings, setShowSettings] = useState(false);

  const refresh = async (groupId = current?.group.id) => {
    if (!groupId) return;
    const state = await api.getGroupState(groupId);
    setCurrent(state);
    setGroups((previous) => {
      const next = previous.some((group) => group.id === state.group.id)
        ? previous.map((group) => group.id === state.group.id ? state.group : group)
        : [state.group, ...previous];
      return next;
    });
  };

  useEffect(() => {
    let disposed = false;
    void (async () => {
      try {
        const boot = await api.bootstrap();
        if (disposed) return;
        setGroups(boot.groups);
        if (boot.groups[0]) await refresh(boot.groups[0].id);
        else setShowCreate(true);
        setSettings(await api.getSettings());
        try { setPresetRoles(await api.getPresetRoles()); } catch {}
      } catch (reason) {
        if (!disposed) setError(readError(reason));
      }
    })();
    const unlisten = listen<ChatEvent>("chat-event", (event) => {
      if (event.payload.groupId !== current?.group.id) return;
      if (event.payload.kind === "message_delta" && event.payload.messageId && event.payload.delta) {
        setCurrent((previous) => previous && ({
          ...previous,
          messages: previous.messages.map((message) => message.id === event.payload.messageId
            ? { ...message, content: message.content + event.payload.delta!, status: "streaming" } : message)
        }));
      } else {
        void refresh(event.payload.groupId).catch((reason) => setError(readError(reason)));
      }
    });
    return () => { disposed = true; void unlisten.then((unsubscribe) => unsubscribe()); };
  }, [current?.group.id]);

  const members = current?.members ?? [];
  const owner = current && members.find((member) => member.id === current.group.ownerMemberId);
  const activeMembers = members.filter((member) => member.isActive);
  const mentionQuery = currentMentionQuery(composer);
  const mentionSuggestions = mentionQuery === null ? [] : activeMembers.filter((member) => member.displayName.toLowerCase().includes(mentionQuery.toLowerCase()));

  const selectGroup = (group: Group) => void refresh(group.id).catch((reason) => setError(readError(reason)));
  const createGroup = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    try {
      const created = await api.createGroup({
        name: String(data.get("name") ?? ""), workspacePath: String(data.get("workspacePath") ?? ""), ownerName: String(data.get("ownerName") ?? ""), presetRoles: selectedRoles.length > 0 ? selectedRoles : undefined
      });
      setCurrent(created); setGroups((previous) => [created.group, ...previous]); setShowCreate(false); setError(null);
    } catch (reason) { setError(readError(reason)); }
  };
  const chooseDirectory = async (field: HTMLInputElement | null) => {
    if (!field) return;
    try { const selected = await open({ directory: true, multiple: false }); if (typeof selected === "string") field.value = selected; }
    catch (reason) { setError(readError(reason)); }
  };
  const handleOcr = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "bmp", "gif", "tiff", "webp"] }]
      });
      if (typeof selected !== "string") return;
      setOcrRunning(true);
      const text = await api.ocrImage(selected);
      setComposer((prev) => prev + text);
      setError(null);
    } catch (reason) {
      setError(readError(reason));
    } finally {
      setOcrRunning(false);
    }
  };
  const send = async () => {
    if (!current || !owner || !composer.trim()) return;
    const body = composer;
    setComposer("");
    try {
      await api.sendMessage(current.group.id, owner.id, body, findMentionedMemberIds(body, activeMembers));
      await refresh(current.group.id);
    } catch (reason) { setComposer(body); setError(readError(reason)); }
  };
  const composerKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey) { event.preventDefault(); void send(); }
  };
  const selectMention = (member: Member) => setComposer((value) => value.replace(/@([^\s@]*)$/u, `@${member.displayName} `));
  const addMember = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); if (!current) return;
    try {
      await api.addMember({ groupId: current.group.id, ...newMember });
      setNewMember(emptyMember); setShowAddMember(false); await refresh();
    } catch (reason) { setError(readError(reason)); }
  };
  const removeMember = async (member: Member) => {
    if (!current || !confirm(`移除 ${member.displayName}？正在运行的任务会被取消。`)) return;
    try { await api.removeMember(current.group.id, member.id); await refresh(); } catch (reason) { setError(readError(reason)); }
  };
  const setAdmin = async (memberId: string | null) => {
    if (!current) return;
    try { setCurrent(await api.setAdmin(current.group.id, memberId)); } catch (reason) { setError(readError(reason)); }
  };
  const detect = async (member: Member) => { try { await api.detectAgent(member.id); await refresh(); } catch (reason) { setError(readError(reason)); } };
  const changeRun = async (run: TaskRun, operation: "cancel" | "retry") => {
    try { if (operation === "cancel") await api.cancelRun(run.id); else await api.retryRun(run.id); await refresh(); } catch (reason) { setError(readError(reason)); }
  };
  const saveSettings = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); if (!settings) return;
    try { setSettings(await api.updateSettings(settings)); setShowSettings(false); } catch (reason) { setError(readError(reason)); }
  };

  const toggleRole = (name: string) => setSelectedRoles((prev) => prev.includes(name) ? prev.filter((r) => r !== name) : [...prev, name]);

  // Fetch preset roles when create modal opens
  useEffect(() => { if (showCreate) { void api.getPresetRoles().then(setPresetRoles).catch(() => {}); setSelectedRoles([]); } }, [showCreate]);

  return <main className="app-shell">
    <aside className="group-sidebar">
      <div className="brand"><span className="brand-mark">L</span><span>Linlis</span></div>
      <div className="sidebar-heading"><span>群聊</span><button className="icon-button" onClick={() => setShowCreate(true)} aria-label="新建群聊">＋</button></div>
      <nav className="group-list">
        {groups.map((group) => <button key={group.id} className={`group-item ${group.id === current?.group.id ? "selected" : ""}`} onClick={() => selectGroup(group)}>
          <span className="group-avatar">{group.name.slice(0, 1)}</span><span className="group-name">{group.name}</span>
        </button>)}
      </nav>
      <div className="sidebar-footer"><button onClick={() => setShowSettings(true)}>运行设置</button></div>
    </aside>

    <section className="chat-panel">
      {current ? <>
        <header className="chat-header"><div><h1>{current.group.name}</h1><p>{activeMembers.length} 名成员 · 本地工作区</p></div><button className="icon-button mobile-members" onClick={() => setShowMembers(!showMembers)}>成员</button></header>
        <div className="message-list">
          {current.messages.length === 0 && <div className="empty-chat"><strong>群聊已创建</strong><span>添加 Agent 后，在消息中输入 <code>@名称</code> 开始协作。</span></div>}
          {current.messages.map((message) => <MessageBubble key={message.id} message={message} members={members} runs={current.runs} ownerId={current.group.ownerMemberId} onRun={changeRun} />)}
        </div>
        <footer className="composer-wrap">
          {mentionSuggestions.length > 0 && <div className="mention-menu">{mentionSuggestions.map((member) => <button key={member.id} onClick={() => selectMention(member)}><Avatar member={member} /><span>{member.displayName}<small>{member.kind === "agent" ? member.roleDescription || member.adapter : "用户"}</small></span></button>)}</div>}
          <textarea value={composer} onChange={(event) => setComposer(event.target.value)} onKeyDown={composerKeyDown} placeholder="发送消息，输入 @ 选择 Agent。Enter 发送，Shift + Enter 换行。" />
          <div className="composer-actions"><span>Agent 会在本机已有登录的 CLI 中运行</span><span><button className="ocr-button" disabled={ocrRunning} title="从图片识别文字" onClick={() => void handleOcr()}>📷</button><button className="send-button" disabled={!composer.trim()} onClick={() => void send()}>发送</button></span></div>
        </footer>
      </> : <div className="loading">正在打开本地群聊…</div>}
    </section>

    {current && showMembers && <aside className="member-panel">
      <header><div><h2>群成员</h2><p>管理员负责未 @ 消息的默认处理</p></div><button className="icon-button" onClick={() => setShowMembers(false)}>×</button></header>
      <div className="member-list">{members.map((member) => <MemberRow key={member.id} member={member} group={current.group} onAdmin={setAdmin} onRemove={removeMember} onDetect={detect} />)}</div>
      {showAddMember ? <form className="add-member-form" onSubmit={addMember}>
        <select value={newMember.kind} onChange={(event) => setNewMember((value) => ({ ...value, kind: event.target.value as NewMember["kind"] }))}><option value="agent">Agent</option><option value="user">用户</option></select>
        <input autoFocus value={newMember.displayName} onChange={(event) => setNewMember((value) => ({ ...value, displayName: event.target.value }))} placeholder="成员名称" required />
        <input value={newMember.roleDescription} onChange={(event) => setNewMember((value) => ({ ...value, roleDescription: event.target.value }))} placeholder={newMember.kind === "agent" ? "职责，例如：代码审查" : "成员说明（可选）"} />
        {newMember.kind === "agent" && <><select value={newMember.adapter} onChange={(event) => setNewMember((value) => ({ ...value, adapter: event.target.value }))}><option value="mock">模拟 Agent（推荐体验）</option><option value="codex">Codex CLI</option><option value="claude-code">Claude Code</option><option value="opencode">OpenCode</option><option value="cursor">Cursor CLI</option></select><input value={newMember.executablePath} onChange={(event) => setNewMember((value) => ({ ...value, executablePath: event.target.value }))} placeholder="可执行文件路径（可选）" /></>}
        <div><button type="button" className="quiet-button" onClick={() => setShowAddMember(false)}>取消</button><button type="submit">添加</button></div>
      </form> : <button className="add-member-button" onClick={() => setShowAddMember(true)}>＋ 添加成员</button>}
    </aside>}

    {showCreate && <Modal title="新建协作群" onClose={() => groups.length > 0 && setShowCreate(false)}><form className="modal-form" onSubmit={createGroup}><label>群名称<input name="name" required placeholder="例如：官网改版" /></label><label>群主名称<input name="ownerName" required defaultValue="我" /></label><label>本地工作目录<div className="path-input"><input name="workspacePath" required placeholder="选择 Agent 可访问的项目目录" /><button type="button" onClick={(event) => void chooseDirectory(event.currentTarget.previousElementSibling as HTMLInputElement)}>浏览</button></div></label><p className="form-hint">此目录由群主明确授权，所有 Agent 任务均在其中执行。</p><button className="primary-wide" type="submit">创建群聊</button></form></Modal>}
    {showSettings && settings && <Modal title="运行设置" onClose={() => setShowSettings(false)}><form className="modal-form" onSubmit={saveSettings}><NumberSetting label="每群并发任务" value={settings.maxConcurrentRuns} onChange={(value) => setSettings({ ...settings, maxConcurrentRuns: value })} min={1} max={8} /><NumberSetting label="任务超时（秒）" value={settings.runTimeoutSeconds} onChange={(value) => setSettings({ ...settings, runTimeoutSeconds: value })} min={30} max={7200} /><NumberSetting label="上下文消息数" value={settings.contextMessageLimit} onChange={(value) => setSettings({ ...settings, contextMessageLimit: value })} min={5} max={200} /><NumberSetting label="管理员最大派生层级" value={settings.maxDelegationDepth} onChange={(value) => setSettings({ ...settings, maxDelegationDepth: value })} min={0} max={4} /><button className="primary-wide" type="submit">保存设置</button></form></Modal>}
    {error && <div className="error-toast"><span>{error}</span><button onClick={() => setError(null)}>×</button></div>}
  </main>;
}

function MessageBubble({ message, members, runs, ownerId, onRun }: { message: GroupState["messages"][number]; members: Member[]; runs: TaskRun[]; ownerId: string; onRun: (run: TaskRun, operation: "cancel" | "retry") => void }) {
  const sender = members.find((member) => member.id === message.senderMemberId);
  const run = runs.find((candidate) => candidate.outputMessageId === message.id);
  const own = message.senderMemberId === ownerId;
  return <article className={`message-row ${own ? "own" : ""}`}><Avatar member={sender} /><div className="message-content"><div className="message-meta"><strong>{sender?.displayName ?? "已移除成员"}</strong><span>{time(message.createdAt)}</span>{run && <Status status={run.status} />}</div><div className={`bubble ${message.status}`}>{message.content || <span className="typing">正在思考<span>···</span></span>}</div>{run?.errorMessage && <p className="run-error">{run.errorMessage}</p>}{run && <div className="run-actions">{(run.status === "running" || run.status === "queued") && <button onClick={() => onRun(run, "cancel")}>停止</button>}{["failed", "cancelled", "interrupted"].includes(run.status) && <button onClick={() => onRun(run, "retry")}>重试</button>}</div>}</div></article>;
}

function MemberRow({ member, group, onAdmin, onRemove, onDetect }: { member: Member; group: Group; onAdmin: (id: string | null) => void; onRemove: (member: Member) => void; onDetect: (member: Member) => void }) {
  const isAdmin = group.adminMemberId === member.id;
  return <div className={`member-row ${member.isActive ? "" : "inactive"}`}><Avatar member={member} /><div className="member-details"><strong>{member.displayName}{member.id === group.ownerMemberId && <em>群主</em>}{isAdmin && <em className="admin-badge">管理员</em>}</strong><span>{member.kind === "agent" ? `${member.adapter} · ${member.runtimeStatus === "ready" ? "已就绪" : member.runtimeStatus === "unavailable" ? "不可用" : "待检测"}` : member.roleDescription || "本地成员"}</span></div>{member.isActive && <div className="member-actions">{member.kind === "agent" && <><button onClick={() => onDetect(member)}>检测</button><button onClick={() => onAdmin(isAdmin ? null : member.id)}>{isAdmin ? "撤销" : "设管理"}</button></>}{member.id !== group.ownerMemberId && <button className="danger" onClick={() => onRemove(member)}>移除</button>}</div>}</div>;
}
function Avatar({ member }: { member?: Member }) { return <span className="avatar" style={{ background: member?.avatarColor ?? "#8792a5" }}>{member?.displayName.slice(0, 1) ?? "?"}</span>; }
function Status({ status }: { status: string }) { return <span className={`status ${status}`}>{({ queued: "排队中", running: "运行中", completed: "完成", failed: "失败", cancelled: "已停止", interrupted: "已中断" } as Record<string, string>)[status] ?? status}</span>; }
function Modal({ title, children, onClose }: { title: string; children: ReactNode; onClose: () => void }) { return <div className="modal-backdrop"><section className="modal"><header><h2>{title}</h2><button className="icon-button" onClick={onClose}>×</button></header>{children}</section></div>; }
function NumberSetting({ label, value, onChange, min, max }: { label: string; value: number; onChange: (value: number) => void; min: number; max: number }) { return <label>{label}<input type="number" min={min} max={max} value={value} onChange={(event) => onChange(Number(event.target.value))} /></label>; }
function readError(reason: unknown) { return typeof reason === "string" ? reason : reason instanceof Error ? reason.message : "发生了未知错误。"; }
