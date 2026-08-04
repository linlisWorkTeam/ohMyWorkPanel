import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { FormEvent, KeyboardEvent, memo, useEffect, useLayoutEffect, useRef, useState, type MouseEvent, type ReactNode } from "react";
import { api, getAuthToken, onUnauthorized, requiresAuth, setAuthToken } from "./api";
import {
  agentReplyDefaultOpen,
  BOTTOM_THRESHOLD_PX,
  extractReplyPreview,
  isNearBottom,
} from "./chatUi";
import { currentMentionQuery, findMentionedMemberIds } from "./mentions";
import { appendChannelDelta, hasRenderableContent, parseMessageContent } from "./messageContent";
import { defaultModelForAdapter, modelsForAdapter } from "./agentModels";
import { chatbotSlotTaken } from "./memberForm";
import { markdownToHtml } from "./markdownLite";
import { isIgnorableWsKind, shouldResyncAfterWsEvent } from "./realtimeWs";
import {
  INITIAL_VISIBLE_MESSAGES,
  OLDER_PAGE_SIZE,
  mergeHotWithOlder,
  nextVisibleCount,
  prependOlderMessages,
  shouldLoadOlderOnScroll,
  sliceVisibleMessages,
} from "./messageHistory";
import { loadAuthUser, resolveSenderMemberId, saveAuthUser, type AuthUser } from "./authSession";
import { loadSendKeyMode, saveSendKeyMode, sendKeyHint, shouldSendOnKey, type SendKeyMode } from "./sendKey";
import type { ChatEvent, Group, GroupState, Member, PresetRole, RuntimeSettings, TaskRun } from "./types";
import { ExperiencePanel } from "./ExperiencePanel";
import { LogsPanel } from "./LogsPanel";
import { ProjectWorkflowView } from "./ProjectWorkflowView";
import { ServerPathPicker } from "./ServerPathPicker";
import { Brand, ThemeSwitcher } from "./theme";

type NewMember = {
  kind: "agent" | "user" | "chatbot";
  displayName: string;
  roleDescription: string;
  adapter: string;
  executablePath: string;
  chatbotProvider: "opencode-go" | "deepseek";
  apiKey: string;
  model: string;
  loginUsername: string;
  loginPassword: string;
};
type Session = "checking" | "login" | "ready";
const emptyMember: NewMember = {
  kind: "agent", displayName: "", roleDescription: "", adapter: "mock", executablePath: "",
  chatbotProvider: "opencode-go", apiKey: "", model: "",
  loginUsername: "", loginPassword: "",
};
const PHASE_LABEL: Record<string, string> = {
  queued: "排队", starting: "启动", preparing: "准备", cli_spawn: "拉起 CLI",
  awaiting_first_token: "等待首包", streaming: "流式输出", finalizing: "收尾",
  completed: "完成", failed: "失败",
};
const time = (value: number) => new Intl.DateTimeFormat("zh-CN", { hour: "2-digit", minute: "2-digit" }).format(value);
const dayLabel = (value: number) => {
  const d = new Date(value);
  const today = new Date(); today.setHours(0, 0, 0, 0);
  const that = new Date(d); that.setHours(0, 0, 0, 0);
  const diff = Math.round((today.getTime() - that.getTime()) / 86400000);
  if (diff === 0) return "今天";
  if (diff === 1) return "昨天";
  return new Intl.DateTimeFormat("zh-CN", { month: "long", day: "numeric", weekday: "short" }).format(d);
};

export function App() {
  // Web: never enter main UI until bootstrap succeeds; stale localStorage token → login.
  const [session, setSession] = useState<Session>(() => (requiresAuth ? "checking" : "ready"));
  const [groups, setGroups] = useState<Group[]>([]);
  const [current, setCurrent] = useState<GroupState | null>(null);
  const [composer, setComposer] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [showMembers, setShowMembers] = useState(() =>
    typeof window === "undefined" || window.matchMedia("(min-width: 1081px)").matches,
  );
  const [rightPanelTab, setRightPanelTab] = useState<"members" | "experiences" | "logs">("members");
  const [mainView, setMainView] = useState<"chat" | "project">("chat");
  const [workspacePath, setWorkspacePath] = useState("/AI/LinlisWorkPanel");
  const [createGroupKind, setCreateGroupKind] = useState<"project" | "chat">("project");
  const [showArchived, setShowArchived] = useState(false);
  const [sendKeyMode, setSendKeyMode] = useState<SendKeyMode>(() => loadSendKeyMode());
  const [authUser, setAuthUser] = useState<AuthUser | null>(() => loadAuthUser());
  const [showAddMember, setShowAddMember] = useState(false);
  const [newMember, setNewMember] = useState<NewMember>(emptyMember);
  const [ocrRunning, setOcrRunning] = useState(false);
  const [ocrPasting, setOcrPasting] = useState(false);
  const [presetRoles, setPresetRoles] = useState<PresetRole[]>([]);
  const [selectedRoles, setSelectedRoles] = useState<string[]>([]);
  const [settings, setSettings] = useState<RuntimeSettings | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [showSidebar, setShowSidebar] = useState(false);
  const [sending, setSending] = useState(false);
  const [detecting, setDetecting] = useState<string | null>(null);
  const [mentionIndex, setMentionIndex] = useState(0);
  const messageListRef = useRef<HTMLDivElement | null>(null);
  const stickToBottom = useRef(true);
  const forceScrollGroupId = useRef<string | null>(null);
  const composerRef = useRef<HTMLTextAreaElement | null>(null);
  const currentGroupIdRef = useRef<string | undefined>(undefined);
  const [showJumpBottom, setShowJumpBottom] = useState(false);
  const [visibleCount, setVisibleCount] = useState(INITIAL_VISIBLE_MESSAGES);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const [hasMoreOlder, setHasMoreOlder] = useState(false);
  const loadingOlderRef = useRef(false);
  currentGroupIdRef.current = current?.group.id;

  const scrollMessagesToBottom = (force = false) => {
    const node = messageListRef.current;
    if (!node) return;
    if (!force && !stickToBottom.current) return;
    const apply = () => {
      node.scrollTop = node.scrollHeight;
      stickToBottom.current = true;
      setShowJumpBottom(false);
    };
    apply();
    requestAnimationFrame(apply);
  };

  const refresh = async (groupId = current?.group.id) => {
    if (!groupId) return;
    const state = await api.getGroupState(groupId);
    setCurrent((prev) => {
      if (!prev || prev.group.id !== state.group.id) {
        setVisibleCount(INITIAL_VISIBLE_MESSAGES);
        setHasMoreOlder(Boolean(state.messagesHasMore));
        return state;
      }
      const merged = mergeHotWithOlder(prev.messages, state.messages);
      setHasMoreOlder(Boolean(state.messagesHasMore));
      return { ...state, messages: merged };
    });
    setGroups((previous) => {
      const next = previous.some((group) => group.id === state.group.id)
        ? previous.map((group) => group.id === state.group.id ? state.group : group)
        : [state.group, ...previous];
      return next;
    });
  };

  const loadOlderMessages = async () => {
    if (!current || loadingOlderRef.current) return;
    const oldest = current.messages[0];
    if (!oldest) return;
    // First expand local hot buffer before hitting archive API.
    if (visibleCount < current.messages.length) {
      loadingOlderRef.current = true;
      setVisibleCount((c) => nextVisibleCount(c, current.messages.length));
      requestAnimationFrame(() => {
        loadingOlderRef.current = false;
      });
      return;
    }
    if (!hasMoreOlder) return;
    loadingOlderRef.current = true;
    setLoadingOlder(true);
    const list = messageListRef.current;
    const prevHeight = list?.scrollHeight ?? 0;
    const prevTop = list?.scrollTop ?? 0;
    try {
      const page = await api.listMessagesBefore(
        current.group.id,
        oldest.createdAt,
        oldest.id,
        OLDER_PAGE_SIZE,
      );
      setCurrent((prev) => {
        if (!prev || prev.group.id !== current.group.id) return prev;
        return { ...prev, messages: prependOlderMessages(prev.messages, page.messages) };
      });
      setHasMoreOlder(page.hasMore);
      setVisibleCount((c) => c + page.messages.length);
      requestAnimationFrame(() => {
        const node = messageListRef.current;
        if (!node) return;
        node.scrollTop = node.scrollHeight - prevHeight + prevTop;
      });
    } catch (reason) {
      setError(readError(reason));
    } finally {
      loadingOlderRef.current = false;
      setLoadingOlder(false);
    }
  };

  const goLogin = (message?: string | null) => {
    setAuthToken(null);
    saveAuthUser(null);
    setAuthUser(null);
    setCurrent(null);
    setGroups([]);
    setSession("login");
    setError(message ?? null);
  };

  // Any API 401 → drop session and show login (covers stale token / Missing token).
  useEffect(() => {
    if (!requiresAuth) return;
    return onUnauthorized(() => goLogin("登录已失效，请重新登录"));
  }, []);

  // Boot: no token → login; has token → bootstrap; failure → login.
  useEffect(() => {
    if (!requiresAuth) return;
    if (session !== "checking") return;
    let disposed = false;
    void (async () => {
      if (!getAuthToken()) {
        if (!disposed) setSession("login");
        return;
      }
      try {
        try {
          const me = await api.verify();
          if (!disposed) {
            const next: AuthUser = {
              userId: me.sub,
              username: me.username,
              isAdmin: Boolean(me.isAdmin ?? me.is_admin),
            };
            saveAuthUser(next);
            setAuthUser(next);
          }
        } catch { /* keep cached */ }
        const boot = await api.bootstrap();
        if (disposed) return;
        setGroups(boot.groups);
        if (boot.groups[0]) {
          forceScrollGroupId.current = boot.groups[0].id;
          await refresh(boot.groups[0].id);
        } else {
          setCurrent(null);
          // Only admins may create groups when empty.
          if ((loadAuthUser()?.isAdmin ?? false)) setShowCreate(true);
        }
        try { setSettings(await api.getSettings()); } catch { /* scoped users may lack settings */ }
        try { setPresetRoles(await api.getPresetRoles()); } catch { /* optional */ }
        if (!disposed) {
          setError(null);
          setSession("ready");
        }
      } catch (reason) {
        if (!disposed) goLogin(isUnauthorizedError(reason) ? "请先登录" : readError(reason));
      }
    })();
    return () => { disposed = true; };
  }, [session]);

  // Desktop bootstrap + event subscription (web bootstrap happens in checking phase).
  useEffect(() => {
    if (session !== "ready") return;
    let disposed = false;
    void (async () => {
      if (requiresAuth) return; // web already bootstrapped in checking phase
      try {
        const boot = await api.bootstrap();
        if (disposed) return;
        setGroups(boot.groups);
        if (boot.groups[0]) {
          forceScrollGroupId.current = boot.groups[0].id;
          await refresh(boot.groups[0].id);
        } else setShowCreate(true);
        setSettings(await api.getSettings());
        try { setPresetRoles(await api.getPresetRoles()); } catch { /* optional */ }
      } catch (reason) {
        if (!disposed) setError(readError(reason));
      }
    })();
    const unlisten = listen<ChatEvent>("chat-event", (event) => {
      const payload = event.payload;
      const activeGroupId = currentGroupIdRef.current;
      if (isIgnorableWsKind(payload.kind)) return;
      if (payload.kind === "orchestration_status") {
        if (activeGroupId) {
          void refresh(activeGroupId).catch((reason) => {
            if (isUnauthorizedError(reason)) goLogin("登录已失效，请重新登录");
            else setError(readError(reason));
          });
        }
        return;
      }
      if (shouldResyncAfterWsEvent(payload.kind)) {
        if (activeGroupId) {
          void refresh(activeGroupId).catch((reason) => {
            if (isUnauthorizedError(reason)) goLogin("登录已失效，请重新登录");
            else setError(readError(reason));
          });
        }
        return;
      }
      const inCurrent = (previous: GroupState | null) =>
        !!previous && (!payload.groupId || payload.groupId === previous.group.id
          || (!!payload.runId && previous.runs.some((r) => r.id === payload.runId)));
      if (payload.kind === "message_delta" && payload.messageId && payload.delta) {
        if (payload.groupId && payload.groupId !== activeGroupId) return;
        const messageId = payload.messageId;
        const delta = payload.delta;
        const channel = payload.channel ?? "final";
        const replace = Boolean(payload.replace);
        setCurrent((previous) => {
          if (!previous || !inCurrent(previous)) return previous;
          const idx = previous.messages.findIndex((m) => m.id === messageId);
          if (idx < 0) {
            void refresh(previous.group.id).catch(() => {});
            return previous;
          }
          const messages = previous.messages.slice();
          const message = messages[idx];
          messages[idx] = {
            ...message,
            content: appendChannelDelta(message.content, channel, delta, replace),
            status: "streaming",
          };
          return { ...previous, messages };
        });
        return;
      }
      if (payload.kind === "run_status" && payload.runId) {
        const terminal = ["completed", "failed", "cancelled", "interrupted"].includes(String(payload.status ?? ""));
        setCurrent((previous) => {
          if (!previous || !inCurrent(previous)) return previous;
          const idx = previous.runs.findIndex((r) => r.id === payload.runId);
          if (idx < 0) {
            void refresh(payload.groupId || previous.group.id).catch((reason) => {
              if (isUnauthorizedError(reason)) goLogin("登录已失效，请重新登录");
              else setError(readError(reason));
            });
            return previous;
          }
          const runs = previous.runs.slice();
          runs[idx] = {
            ...runs[idx],
            status: (payload.status as TaskRun["status"]) ?? runs[idx].status,
            outputMessageId: payload.messageId ?? runs[idx].outputMessageId,
            errorMessage: payload.error ?? runs[idx].errorMessage,
            phase: payload.phase ?? runs[idx].phase,
            phaseUpdatedAt: Date.now(),
          };
          let messages = previous.messages;
          const outputId = runs[idx].outputMessageId;
          if (terminal && outputId) {
            const mi = messages.findIndex((m) => m.id === outputId);
            if (mi >= 0 && messages[mi].status === "streaming") {
              messages = messages.slice();
              messages[mi] = {
                ...messages[mi],
                status: payload.status === "completed" ? "completed" : (payload.status ?? messages[mi].status),
              };
            }
          }
          return { ...previous, runs, messages };
        });
        // Terminal status: resync from server so missed deltas still appear without a full reload.
        if (terminal) {
          void refresh(payload.groupId || activeGroupId).catch((reason) => {
            if (isUnauthorizedError(reason)) goLogin("登录已失效，请重新登录");
            else setError(readError(reason));
          });
        }
        return;
      }
      if (payload.groupId && payload.groupId !== activeGroupId) return;
      void refresh(payload.groupId || activeGroupId).catch((reason) => {
        if (isUnauthorizedError(reason)) goLogin("登录已失效，请重新登录");
        else setError(readError(reason));
      });
    });
    return () => { disposed = true; void unlisten.then((unsubscribe) => unsubscribe()); };
  }, [session]);

  // Must stay above any conditional return — hooks order cannot change across login/ready.
  useEffect(() => {
    if (showCreate) {
      void api.getPresetRoles().then(setPresetRoles).catch(() => {});
      setSelectedRoles([]);
    }
  }, [showCreate]);

  // Reset mention highlight whenever the suggestion set changes.
  const mentionQuery = currentMentionQuery(composer);
  useEffect(() => { setMentionIndex(0); }, [mentionQuery, current?.group.id]);

  // Scroll: enter group → last line; stick while near bottom on new content.
  const lastMessage = current?.messages[current.messages.length - 1];
  const lastKey = `${current?.group.id}:${current?.messages.length}:${lastMessage?.content.length ?? 0}:${mainView}`;
  useLayoutEffect(() => {
    if (mainView !== "chat") return;
    const groupId = current?.group.id;
    if (!groupId) return;
    const entering = forceScrollGroupId.current === groupId;
    if (entering) forceScrollGroupId.current = null;
    scrollMessagesToBottom(entering || stickToBottom.current);
  }, [lastKey, mainView, current?.group.id]);

  const handleMessageScroll = () => {
    const node = messageListRef.current;
    if (!node) return;
    const near = isNearBottom(node.scrollTop, node.scrollHeight, node.clientHeight, BOTTOM_THRESHOLD_PX);
    stickToBottom.current = near;
    setShowJumpBottom(!near);
    if (shouldLoadOlderOnScroll(node.scrollTop) && !loadingOlderRef.current) {
      void loadOlderMessages();
    }
  };

  const selectGroup = (group: Group) => {
    forceScrollGroupId.current = group.id;
    stickToBottom.current = true;
    setShowJumpBottom(false);
    setVisibleCount(INITIAL_VISIBLE_MESSAGES);
    setHasMoreOlder(false);
    setComposer("");
    setShowSidebar(false);
    setMainView("chat");
    void refresh(group.id).catch((reason) => setError(readError(reason)));
  };

  if (requiresAuth && session === "checking") {
    return <main className="auth-screen"><div className="auth-card"><Brand /><p className="auth-hint">正在检查登录状态…</p></div></main>;
  }

  if (requiresAuth && session === "login") {
    return <AuthScreen
      error={error}
      onError={setError}
      onAuthed={(user) => {
        setAuthUser(user);
        setError(null);
        setSession("checking");
      }}
    />;
  }

  const members = current?.members ?? [];
  const chatbotTaken = chatbotSlotTaken(current?.group, members);
  const isChatGroup = current?.group.groupKind === "chat";
  const isAdmin = authUser?.isAdmin ?? !requiresAuth;
  const owner = current && members.find((member) => member.id === current.group.ownerMemberId);
  const senderMemberId = current
    ? resolveSenderMemberId(members, current.group.ownerMemberId, authUser?.userId, isAdmin)
    : null;
  const activeMembers = members.filter((member) => member.isActive);
  const addMemberKind = chatbotTaken && newMember.kind === "chatbot" ? "agent" : newMember.kind;
  const activeGroups = groups.filter((g) => !g.archived);
  const archivedGroups = groups.filter((g) => g.archived);
  const mentionSuggestions = mentionQuery === null ? [] : activeMembers.filter((member) => member.displayName.toLowerCase().includes(mentionQuery.toLowerCase())).slice(0, 8);
  const allMessages = current?.messages ?? [];
  const visibleMessages = sliceVisibleMessages(allMessages, visibleCount);
  const totalHint = current?.messagesTotal ?? allMessages.length;

  const createGroup = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    try {
      const created = await api.createGroup({
        name: String(data.get("name") ?? ""),
        workspacePath: createGroupKind === "chat" ? "" : workspacePath.trim(),
        ownerName: String(data.get("ownerName") ?? ""),
        presetRoles: createGroupKind === "project" && selectedRoles.length > 0 ? selectedRoles : undefined,
        groupKind: createGroupKind,
      });
      setCurrent(created); setGroups((previous) => [created.group, ...previous]); setShowCreate(false); setError(null); setMainView("chat");
      setCreateGroupKind("project");
    } catch (reason) { setError(readError(reason)); }
  };
  const archiveGroup = async (group: Group, archived: boolean, event?: MouseEvent) => {
    event?.stopPropagation();
    try {
      const updated = await api.setGroupArchived(group.id, archived);
      setGroups((prev) => prev.map((g) => (g.id === updated.id ? { ...g, ...updated } : g)));
      if (current?.group.id === updated.id) {
        setCurrent((prev) => prev && ({ ...prev, group: { ...prev.group, ...updated } }));
      }
    } catch (reason) { setError(readError(reason)); }
  };
  const changeMemberModel = async (member: Member, model: string) => {
    try {
      const updated = await api.updateMemberModel(member.id, model || null);
      setCurrent((prev) => {
        if (!prev) return prev;
        return { ...prev, members: prev.members.map((m) => (m.id === updated.id ? { ...m, ...updated } : m)) };
      });
    } catch (reason) { setError(readError(reason)); }
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
  const handlePaste = async (event: React.ClipboardEvent<HTMLTextAreaElement>) => {
    const items = event.clipboardData?.items;
    if (!items) return;
    for (let i = 0; i < items.length; i++) {
      const item = items[i];
      if (item.type.startsWith("image/")) {
        event.preventDefault();
        const file = item.getAsFile();
        if (!file) continue;
        setOcrPasting(true);
        try {
          const reader = new FileReader();
          const base64 = await new Promise<string>((resolve, reject) => {
            reader.onload = () => resolve(reader.result as string);
            reader.onerror = () => reject(reader.error);
            reader.readAsDataURL(file);
          });
          const text = await api.ocrImageBase64(base64);
          if (text.trim()) setComposer((prev) => prev + text);
          setError(null);
        } catch (reason) {
          setError(readError(reason));
        } finally {
          setOcrPasting(false);
        }
        break;
      }
    }
  };

  const send = async () => {
    if (!current || !composer.trim() || sending) return;
    if (!senderMemberId) {
      setError(isAdmin ? "当前群缺少群主成员，无法发送" : "未找到你在本群的成员身份，无法发言");
      return;
    }
    const body = composer;
    setComposer("");
    setSending(true);
    try {
      await api.sendMessage(current.group.id, senderMemberId, body, findMentionedMemberIds(body, activeMembers));
      await refresh(current.group.id);
    } catch (reason) { setComposer(body); setError(readError(reason)); }
    finally { setSending(false); composerRef.current?.focus(); }
  };
  const composerKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    // Don't send while an IME (e.g. 中文输入法) is composing a selection.
    if (event.nativeEvent.isComposing) return;
    if (mentionSuggestions.length > 0) {
      if (event.key === "ArrowDown") { event.preventDefault(); setMentionIndex((i) => (i + 1) % mentionSuggestions.length); return; }
      if (event.key === "ArrowUp") { event.preventDefault(); setMentionIndex((i) => (i - 1 + mentionSuggestions.length) % mentionSuggestions.length); return; }
      if (event.key === "Tab" || event.key === "Enter") { event.preventDefault(); selectMention(mentionSuggestions[Math.min(mentionIndex, mentionSuggestions.length - 1)]); return; }
      if (event.key === "Escape") { event.preventDefault(); setComposer((value) => value.replace(/@([^\s@]*)$/u, "")); return; }
    }
    if (shouldSendOnKey(sendKeyMode, event.key, event.shiftKey, event.ctrlKey, event.metaKey)) {
      event.preventDefault();
      void send();
    }
  };
  const selectMention = (member: Member) => setComposer((value) => value.replace(/@([^\s@]*)$/u, `@${member.displayName} `));
  const addMember = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); if (!current) return;
    try {
      await api.addMember({
        groupId: current.group.id,
        kind: newMember.kind,
        displayName: newMember.displayName,
        roleDescription: newMember.roleDescription,
        adapter: newMember.kind === "agent" ? newMember.adapter : undefined,
        executablePath: newMember.kind === "agent" ? newMember.executablePath : undefined,
        chatbotProvider: newMember.kind === "chatbot" ? newMember.chatbotProvider : undefined,
        apiKey: newMember.kind === "chatbot" ? newMember.apiKey : undefined,
        model: newMember.kind === "agent" || newMember.kind === "chatbot"
          ? (newMember.model || defaultModelForAdapter(
              newMember.kind === "chatbot"
                ? (newMember.chatbotProvider === "deepseek" ? "chatbot-deepseek" : "chatbot-opencode-go")
                : newMember.adapter,
            ) || undefined)
          : undefined,
        loginUsername: newMember.kind === "user" ? newMember.loginUsername.trim() : undefined,
        loginPassword: newMember.kind === "user" ? newMember.loginPassword : undefined,
      });
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
  const detect = async (member: Member) => {
    if (detecting) return;
    setDetecting(member.id);
    try { await api.detectAgent(member.id); await refresh(); } catch (reason) { setError(readError(reason)); }
    finally { setDetecting(null); }
  };
  const changeRun = async (run: TaskRun, operation: "cancel" | "retry") => {
    try { if (operation === "cancel") await api.cancelRun(run.id); else await api.retryRun(run.id); await refresh(); } catch (reason) { setError(readError(reason)); }
  };
  const saveSettings = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); if (!settings) return;
    try { setSettings(await api.updateSettings(settings)); setShowSettings(false); } catch (reason) { setError(readError(reason)); }
  };

  const toggleRole = (name: string) => setSelectedRoles((prev) => prev.includes(name) ? prev.filter((r) => r !== name) : [...prev, name]);

  const openSidebar = () => { setShowMembers(false); setShowSidebar(true); };
  const toggleMembers = () => {
    setShowMembers((open) => {
      const next = !open;
      if (next) setShowSidebar(false);
      return next;
    });
  };

  return <main className="app-shell">
    {showSidebar && <div className="sidebar-backdrop" onClick={() => setShowSidebar(false)} />}
    {showMembers && <div className="members-backdrop" onClick={() => setShowMembers(false)} />}
    <aside className={`group-sidebar ${showSidebar ? "open" : ""}`}>
      <Brand />
      <div className="sidebar-heading"><span>群聊</span>{isAdmin && <button className="icon-button" onClick={() => setShowCreate(true)} aria-label="新建群聊">＋</button>}</div>
      <nav className="group-list">
        {activeGroups.map((group) => (
          <div key={group.id} className={`group-item-row ${group.id === current?.group.id ? "selected" : ""}`}>
            <button type="button" className="group-item" onClick={() => selectGroup(group)}>
              <span className="group-avatar">{group.name.slice(0, 1)}</span>
              <span className="group-name">{group.name}{group.groupKind === "chat" ? " · 聊" : ""}</span>
            </button>
            {isAdmin && <button type="button" className="group-archive-btn" title="归档群组" aria-label="归档群组" onClick={(e) => void archiveGroup(group, true, e)}>−</button>}
          </div>
        ))}
        {archivedGroups.length > 0 && (
          <div className="archived-section">
            <button type="button" className="archived-toggle" onClick={() => setShowArchived((v) => !v)}>
              {showArchived ? "▾" : "▸"} 已归档（{archivedGroups.length}）
            </button>
            {showArchived && archivedGroups.map((group) => (
              <div key={group.id} className={`group-item-row archived ${group.id === current?.group.id ? "selected" : ""}`}>
                <button type="button" className="group-item" onClick={() => selectGroup(group)}>
                  <span className="group-avatar">{group.name.slice(0, 1)}</span>
                  <span className="group-name">{group.name}</span>
                </button>
                <button type="button" className="group-archive-btn" title="取消归档" aria-label="取消归档" onClick={(e) => void archiveGroup(group, false, e)}>+</button>
              </div>
            ))}
          </div>
        )}
      </nav>
      <div className="sidebar-footer">
        <ThemeSwitcher />
        {isAdmin && <button onClick={() => setShowSettings(true)}>运行设置</button>}
        {requiresAuth && <button onClick={() => goLogin(null)}>退出登录{authUser ? `（${authUser.username}）` : ""}</button>}
      </div>
    </aside>

    <section className="chat-panel">
      {current ? <>
        <header className="chat-header">
          <div className="chat-header-main">
            <button className="icon-button mobile-nav" onClick={openSidebar} aria-label="群列表">☰</button>
            <div>
              <div className="group-title-row">
                <h1>{current.group.name}</h1>
                {!isChatGroup && isAdmin && (
                  <div className="view-toggle" role="group" aria-label="主视图">
                    <button type="button" className={mainView === "chat" ? "active" : ""} onClick={() => setMainView("chat")}>聊天</button>
                    <button type="button" className={mainView === "project" ? "active" : ""} onClick={() => setMainView("project")}>项目</button>
                  </div>
                )}
              </div>
              <p>{activeMembers.length} 名成员 · {isChatGroup ? "聊天群" : "项目群 · 服务器工作区"}</p>
            </div>
          </div>
          <button className="icon-button mobile-members" onClick={toggleMembers} aria-label="成员面板">成员</button>
        </header>
        {!isChatGroup && mainView === "project" ? (
          <ProjectWorkflowView
            group={current.group}
            members={members}
            runs={current.runs}
            canManage
            onGroupPatch={(g) => setCurrent((prev) => prev && ({ ...prev, group: { ...prev.group, ...g } }))}
            onMemberPatch={(m) => setCurrent((prev) => {
              if (!prev) return prev;
              const members = prev.members.map((x) => (x.id === m.id ? { ...x, ...m } : x));
              return { ...prev, members };
            })}
            onError={(msg) => setError(msg)}
          />
        ) : <>
        <div className="message-list-shell">
        <div className="message-list" ref={messageListRef} onScroll={handleMessageScroll}>
          {current.messages.length === 0 && <div className="empty-chat"><strong>{current.group.name}</strong><span>{isChatGroup ? "添加聊天机器人后，输入 @名称 开始对话。" : "添加 Agent 后，在消息中输入 @名称 开始协作。"}</span><span className="empty-chat-sub">{sendKeyHint(sendKeyMode)} · @ 触发成员菜单</span></div>}
          {(hasMoreOlder || visibleCount < allMessages.length || loadingOlder) && (
            <div className="day-divider history-load-hint">
              <span>
                {loadingOlder
                  ? "加载更早消息…"
                  : `上滑加载更早 · 显示 ${visibleMessages.length}/${totalHint}`}
              </span>
            </div>
          )}
          {visibleMessages.map((message, index) => {
            const prev = index > 0 ? visibleMessages[index - 1] : null;
            const showDay = !prev || dayLabel(prev.createdAt) !== dayLabel(message.createdAt);
            return (
              <div key={message.id} className="day-block">
                {showDay && <div className="day-divider"><span>{dayLabel(message.createdAt)}</span></div>}
                <MessageBubble message={message} members={members} runs={current.runs} viewerMemberId={senderMemberId} onRun={changeRun} />
              </div>
            );
          })}
          {current.runs
            .filter((run) => (run.status === "queued" || run.status === "running") && (!run.outputMessageId || !current.messages.some((message) => message.id === run.outputMessageId)))
            .map((run) => {
              const agent = members.find((member) => member.id === run.agentMemberId);
              return (
                <div key={`pending-${run.id}`} className="message-row is-responding">
                  <Avatar member={agent} responding />
                  <div className="message-content">
                    <div className="message-meta"><strong>{agent?.displayName ?? "Agent"}</strong><Status status={run.status} />{run.phase && <em className="phase-badge">{PHASE_LABEL[run.phase] ?? run.phase}</em>}</div>
                    <div className="bubble streaming"><TypingIndicator label={run.phase ? (PHASE_LABEL[run.phase] ?? run.phase) : run.status === "queued" ? "排队中" : "…"} /></div>
                  </div>
                </div>
              );
            })}
        </div>
        {showJumpBottom && (
          <button
            type="button"
            className="jump-bottom-btn"
            onClick={() => scrollMessagesToBottom(true)}
            title="回到最下方"
          >
            ↓ 最新
          </button>
        )}
        </div>
        <footer className="composer-wrap">
          {mentionSuggestions.length > 0 && (
            <div className="mention-menu">
              {mentionSuggestions.map((member, index) => (
                <button key={member.id} className={index === mentionIndex ? "mention-active" : ""} onMouseEnter={() => setMentionIndex(index)} onClick={() => selectMention(member)}>
                  <Avatar member={member} />
                  <span>{member.displayName}<small>{member.kind === "agent" ? member.roleDescription || member.adapter : member.kind === "chatbot" ? "聊天机器人" : "用户"}</small></span>
                </button>
              ))}
            </div>
          )}
          <textarea ref={composerRef} value={composer} onChange={(event) => setComposer(event.target.value)} onKeyDown={composerKeyDown} onPaste={handlePaste} placeholder={`发送消息，输入 @ 选择成员。${sendKeyHint(sendKeyMode)}`} />
          <div className="composer-actions">
            <span>{sending ? "发送中…" : (isChatGroup ? "聊天模式" : "Agent 在服务器工作目录中运行")}</span>
            <span className="composer-actions-right">
              <button
                type="button"
                className="quiet-button send-key-toggle"
                title="切换发送快捷键"
                onClick={() => {
                  const next: SendKeyMode = sendKeyMode === "enter" ? "ctrlEnter" : "enter";
                  setSendKeyMode(next);
                  saveSendKeyMode(next);
                }}
              >
                {sendKeyMode === "enter" ? "↵ Enter" : "⌃↵ Ctrl"}
              </button>
              <button className="ocr-button" disabled={ocrRunning || ocrPasting} title={ocrPasting ? "正在识别粘贴的图片…" : "从图片识别文字"} onClick={() => void handleOcr()}>{ocrPasting ? "…" : "📷"}</button>
              <button className="send-button" disabled={!composer.trim() || sending} onClick={() => void send()}>{sending ? "发送中" : "发送"}</button>
            </span>
          </div>
        </footer>
        </>}
      </> : <div className="loading">正在打开本地群聊…</div>}
    </section>

    {current && showMembers && <aside className="member-panel">
      <header>
        <div className="pm-tab-bar">
          <button className={`pm-tab-btn ${rightPanelTab === "members" ? "active" : ""}`} onClick={() => setRightPanelTab("members")}>群成员</button>
          {!isChatGroup && isAdmin && <button className={`pm-tab-btn`} onClick={() => { setMainView("project"); }}>项目管理</button>}
          {isAdmin && <button className={`pm-tab-btn ${rightPanelTab === "experiences" ? "active" : ""}`} onClick={() => setRightPanelTab("experiences")}>经验</button>}
          {isAdmin && <button className={`pm-tab-btn ${rightPanelTab === "logs" ? "active" : ""}`} onClick={() => setRightPanelTab("logs")}>日志</button>}
        </div>
        <button className="icon-button" onClick={() => setShowMembers(false)}>×</button>
      </header>
      {rightPanelTab === "members" ? <>
        {(current.group.announcement ?? "").trim() && (
          <div className="announce-banner" title={current.group.announcement}>
            公告：{(current.group.announcement ?? "").slice(0, 120)}{(current.group.announcement ?? "").length > 120 ? "…" : ""}
          </div>
        )}
        <div className="member-list">{members.map((member) => {
          const responding = current.runs.some((run) => run.agentMemberId === member.id && (run.status === "queued" || run.status === "running"));
          return <MemberRow key={member.id} member={member} group={current.group} responding={responding} detecting={detecting === member.id} onAdmin={setAdmin} onRemove={removeMember} onDetect={detect} onModel={(m, model) => void changeMemberModel(m, model)} />;
        })}</div>
        {showAddMember ? <form className="add-member-form" onSubmit={addMember}>
          <select
            value={addMemberKind}
            onChange={(event) => {
              const kind = event.target.value as NewMember["kind"];
              if (kind === "chatbot" && chatbotTaken) return;
              setNewMember((value) => ({ ...value, kind }));
            }}
          >
            <option value="agent">Agent</option>
            <option value="user">用户</option>
            <option value="chatbot" disabled={chatbotTaken}>
              {chatbotTaken ? "聊天机器人（项目群已有）" : "聊天机器人"}
            </option>
          </select>
          {chatbotTaken && (
            <p className="form-hint">项目群限 1 个聊天机器人；聊天群可添加多个。</p>
          )}
          <input autoFocus value={newMember.displayName} onChange={(event) => setNewMember((value) => ({ ...value, displayName: event.target.value }))} placeholder="成员显示名称" required />
          <input value={newMember.roleDescription} onChange={(event) => setNewMember((value) => ({ ...value, roleDescription: event.target.value }))} placeholder={addMemberKind === "agent" ? "职责，例如：代码审查" : addMemberKind === "chatbot" ? "机器人说明（可选）" : "成员说明（可选）"} />
          {addMemberKind === "user" && <>
            <input value={newMember.loginUsername} onChange={(event) => setNewMember((value) => ({ ...value, loginUsername: event.target.value }))} placeholder="登录用户名" required autoComplete="off" />
            <input type="password" value={newMember.loginPassword} onChange={(event) => setNewMember((value) => ({ ...value, loginPassword: event.target.value }))} placeholder="登录密码" required autoComplete="new-password" />
            <p className="form-hint">对方用该用户名/密码登录后，只能看到并进入本群对话。</p>
          </>}
          {addMemberKind === "agent" && <>
            <select value={newMember.adapter} onChange={(event) => {
              const adapter = event.target.value;
              setNewMember((value) => ({ ...value, adapter, model: defaultModelForAdapter(adapter) }));
            }}>
              <option value="mock">模拟 Agent（推荐体验）</option>
              <option value="codex">Codex CLI</option>
              <option value="openclaw">OpenClaw</option>
              <option value="cursor">Cursor CLI（agent/cursor-agent）</option>
              <option value="claude-code">Claude Code</option>
              <option value="opencode">OpenCode</option>
            </select>
            {modelsForAdapter(newMember.adapter).length > 0 && (
              <select value={newMember.model || defaultModelForAdapter(newMember.adapter)} onChange={(event) => setNewMember((value) => ({ ...value, model: event.target.value }))}>
                {modelsForAdapter(newMember.adapter).map((m) => <option key={m} value={m}>{m}</option>)}
              </select>
            )}
            <input value={newMember.executablePath} onChange={(event) => setNewMember((value) => ({ ...value, executablePath: event.target.value }))} placeholder="可执行文件路径（可选）" />
          </>}
          {addMemberKind === "chatbot" && <>
            <select value={newMember.chatbotProvider} onChange={(event) => {
              const chatbotProvider = event.target.value as NewMember["chatbotProvider"];
              const adapter = chatbotProvider === "deepseek" ? "chatbot-deepseek" : "chatbot-opencode-go";
              setNewMember((value) => ({ ...value, chatbotProvider, model: defaultModelForAdapter(adapter) }));
            }}>
              <option value="opencode-go">OpenCode Go</option>
              <option value="deepseek">DeepSeek 官方</option>
            </select>
            <select
              value={newMember.model || defaultModelForAdapter(newMember.chatbotProvider === "deepseek" ? "chatbot-deepseek" : "chatbot-opencode-go")}
              onChange={(event) => setNewMember((value) => ({ ...value, model: event.target.value }))}
            >
              {modelsForAdapter(newMember.chatbotProvider === "deepseek" ? "chatbot-deepseek" : "chatbot-opencode-go").map((m) => (
                <option key={m} value={m}>{m}</option>
              ))}
            </select>
            <input type="password" autoComplete="off" value={newMember.apiKey} onChange={(event) => setNewMember((value) => ({ ...value, apiKey: event.target.value }))} placeholder="API Key（仅存服务器，不进 git）" required />
            <p className="form-hint">{isChatGroup ? "聊天群可添加多个机器人。" : "项目群仅可添加一个聊天机器人。"}</p>
          </>}
          <div><button type="button" className="quiet-button" onClick={() => setShowAddMember(false)}>取消</button><button type="submit">添加</button></div>
        </form> : isAdmin ? <button className="add-member-button" onClick={() => { setNewMember(emptyMember); setShowAddMember(true); }}>＋ 添加成员</button> : null}
      </> : rightPanelTab === "experiences" ? <ExperiencePanel groupId={current.group.id} members={members} ownerId={current.group.ownerMemberId} onError={(msg) => setError(msg)} />
      : <LogsPanel onError={(msg) => setError(msg)} />}
    </aside>}

    {showCreate && (
      <Modal title="新建群组" onClose={() => groups.length > 0 && setShowCreate(false)}>
        <form className="modal-form" onSubmit={createGroup}>
          <label>群类型
            <select value={createGroupKind} onChange={(e) => setCreateGroupKind(e.target.value as "project" | "chat")}>
              <option value="chat">聊天群（多机器人，无项目功能）</option>
              <option value="project">项目群（工作区 + 路线图/编排）</option>
            </select>
          </label>
          <label>群名称<input name="name" required placeholder={createGroupKind === "chat" ? "例如：日常闲聊" : "例如：官网改版"} /></label>
          <label>群主名称<input name="ownerName" required defaultValue="我" /></label>
          {createGroupKind === "project" && (
            <label>服务器工作目录
              <ServerPathPicker value={workspacePath} onChange={setWorkspacePath} onError={setError} />
            </label>
          )}
          {createGroupKind === "project" && presetRoles.length > 0 && (
            <div className="preset-roles">
              <span className="preset-roles-label">预置 Agent 角色</span>
              <div className="preset-roles-grid">
                {presetRoles.map((role) => {
                  const selected = selectedRoles.includes(role.name);
                  return (
                    <button key={role.name} type="button" className={`preset-role ${selected ? "selected" : ""}`} onClick={() => toggleRole(role.name)}>
                      <span className="preset-role-dot" style={{ background: role.avatarColor }} />
                      <span className="preset-role-body">
                        <strong>{role.name}</strong>
                        <small>{role.adapter}{role.roleDescription ? ` · ${role.roleDescription}` : ""}</small>
                      </span>
                      <span className="preset-role-check">{selected ? "✓" : "+"}</span>
                    </button>
                  );
                })}
              </div>
            </div>
          )}
          <p className="form-hint">{createGroupKind === "chat" ? "聊天群无需工作区，创建后可添加多个聊天机器人。" : "必须选择服务器上已存在的绝对路径；所有 Agent 任务均在该目录执行。"}</p>
          <button className="primary-wide" type="submit">创建{createGroupKind === "chat" ? "聊天群" : "项目群"}</button>
        </form>
      </Modal>
    )}
    {showSettings && settings && <Modal title="运行设置" onClose={() => setShowSettings(false)}><form className="modal-form" onSubmit={saveSettings}><NumberSetting label="每群并发任务" value={settings.maxConcurrentRuns} onChange={(value) => setSettings({ ...settings, maxConcurrentRuns: value })} min={1} max={8} /><NumberSetting label="任务超时（秒）" value={settings.runTimeoutSeconds} onChange={(value) => setSettings({ ...settings, runTimeoutSeconds: value })} min={30} max={7200} /><NumberSetting label="上下文消息数" value={settings.contextMessageLimit} onChange={(value) => setSettings({ ...settings, contextMessageLimit: value })} min={5} max={200} /><NumberSetting label="管理员最大派生层级" value={settings.maxDelegationDepth} onChange={(value) => setSettings({ ...settings, maxDelegationDepth: value })} min={0} max={4} /><button className="primary-wide" type="submit">保存设置</button></form></Modal>}
    {error && <div className="error-toast"><span>{error}</span><button onClick={() => setError(null)}>×</button></div>}
  </main>;
}

const MessageBubble = memo(function MessageBubble({ message, members, runs, viewerMemberId, onRun }: { message: GroupState["messages"][number]; members: Member[]; runs: TaskRun[]; viewerMemberId: string | null; onRun: (run: TaskRun, operation: "cancel" | "retry") => void }) {
  const sender = members.find((member) => member.id === message.senderMemberId);
  const run = runs.find((candidate) => candidate.outputMessageId === message.id);
  const own = Boolean(viewerMemberId) && message.senderMemberId === viewerMemberId;
  const responding = message.status === "streaming" || run?.status === "queued" || run?.status === "running";
  const hasContent = hasRenderableContent(message.content);
  const foldAgent = !own && hasContent;
  const [expanded, setExpanded] = useState(() => agentReplyDefaultOpen(responding));
  useEffect(() => {
    if (responding) setExpanded(true);
  }, [responding]);
  const body = hasContent ? (
    <>
      <MessagePartsBody content={message.content} streaming={responding} />
      {responding && <span className="stream-caret" aria-hidden />}
    </>
  ) : (
    <TypingIndicator label={
      run?.phase ? (PHASE_LABEL[run.phase] ?? run.phase)
        : run?.status === "queued" ? "排队中" : "…"
    } />
  );
  return (
    <article className={`message-row ${own ? "own" : ""} ${responding ? "is-responding" : ""}`}>
      <Avatar member={sender} responding={responding && !own} />
      <div className="message-content">
        <div className="message-meta">
          <strong>{sender?.displayName ?? "已移除成员"}</strong>
          <span>{time(message.createdAt)}</span>
          {run && <Status status={run.status} />}
          {run?.phase && (run.status === "queued" || run.status === "running") && (
            <em className="phase-badge">{PHASE_LABEL[run.phase] ?? run.phase}</em>
          )}
          {run?.reviewStatus && <ReviewBadge reviewStatus={run.reviewStatus} />}
        </div>
        <div className={`bubble ${message.status}${responding ? " streaming" : ""}`}>
          {foldAgent ? (
            <details
              className="agent-reply-fold"
              open={expanded}
              onToggle={(event) => setExpanded(event.currentTarget.open)}
            >
              <summary>{responding ? "流式输出中…" : extractReplyPreview(message.content)}</summary>
              {body}
            </details>
          ) : (
            body
          )}
        </div>
        {run?.errorMessage && <p className="run-error">{run.errorMessage}</p>}
        {run && (
          <div className="run-actions">
            {(run.status === "running" || run.status === "queued") && <button onClick={() => onRun(run, "cancel")}>停止</button>}
            {["failed", "cancelled", "interrupted", "changes_requested"].includes(run.status) && <button onClick={() => onRun(run, "retry")}>重试</button>}
          </div>
        )}
      </div>
    </article>
  );
});

function MessagePartsBody({ content, streaming }: { content: string; streaming: boolean }) {
  const doc = parseMessageContent(content);
  if (!doc) {
    return <MarkdownBlock text={content} />;
  }
  const thinking = doc.parts.find((part) => part.channel === "thinking" && part.text.trim());
  const artifact = doc.parts.find((part) => part.channel === "artifact" && part.text.trim());
  const finals = doc.parts.filter((part) => part.channel === "final" && part.text.trim());
  return (
    <div className="message-parts">
      {thinking && (
        <details className="part-thinking" open={streaming || undefined}>
          <summary>思考过程{streaming ? "（生成中）" : ""}</summary>
          <pre>{thinking.text}</pre>
        </details>
      )}
      {artifact && (
        <details className="part-artifact" open={streaming || undefined}>
          <summary>中间产物{streaming ? "（生成中）" : ""}</summary>
          <pre>{artifact.text}</pre>
        </details>
      )}
      {finals.map((part, index) => (
        <MarkdownBlock key={`final-${index}`} text={part.text} />
      ))}
    </div>
  );
}

function MarkdownBlock({ text }: { text: string }) {
  return (
    <div
      className="part-final md-body"
      dangerouslySetInnerHTML={{ __html: markdownToHtml(text) }}
    />
  );
}

function TypingIndicator({ label }: { label: string }) {
  return (
    <span className="typing" role="status" aria-live="polite">
      <span className="typing-label">{label}</span>
      <span className="typing-dots" aria-hidden><i /><i /><i /></span>
    </span>
  );
}

function MemberRow({ member, group, responding, detecting, onAdmin, onRemove, onDetect, onModel }: {
  member: Member; group: Group; responding?: boolean; detecting?: boolean;
  onAdmin: (id: string | null) => void; onRemove: (member: Member) => void; onDetect: (member: Member) => void;
  onModel: (member: Member, model: string) => void;
}) {
  const isAdmin = group.adminMemberId === member.id;
  const modelOptions = modelsForAdapter(member.adapter);
  const statusText = member.kind === "agent"
    ? `${member.adapter} · ${detecting ? "检测中…" : responding ? "生成回复中" : member.runtimeStatus === "ready" ? "已就绪" : member.runtimeStatus === "unavailable" ? "不可用" : "待检测"}${member.keepAlive ? ` · 保活${member.warmStatus ? `(${member.warmStatus})` : ""}` : ""}`
    : member.kind === "chatbot"
      ? `${member.adapter ?? "chatbot"} · ${member.model || "deepseek-v4-flash"} · ${member.apiKeySet ? "已配置 Key" : "缺 Key"}`
      : member.roleDescription || "本地成员";
  return (
    <div className={`member-row ${member.isActive ? "" : "inactive"} ${responding ? "is-responding" : ""}`}>
      <Avatar member={member} responding={responding} />
      <div className="member-details">
        <strong>
          {member.displayName}
          {member.id === group.ownerMemberId && <em>群主</em>}
          {isAdmin && <em className="admin-badge">管理员</em>}
          {member.kind === "chatbot" && <em className="admin-badge">机器人</em>}
          {responding && <em className="responding-badge">回应中</em>}
        </strong>
        <span>
          {statusText}
          {member.tags ? ` · 🏷 ${member.tags}` : ""}
        </span>
        {modelOptions.length > 0 && (member.kind === "agent" || member.kind === "chatbot") && (
          <select
            className="member-model-select"
            value={member.model || modelOptions[0]}
            onChange={(e) => onModel(member, e.target.value)}
            title="切换模型"
          >
            {modelOptions.map((m) => <option key={m} value={m}>{m}</option>)}
          </select>
        )}
      </div>
      <div className="member-actions">
        {member.kind === "agent" && (
          <>
            <button disabled={!!detecting} onClick={() => onDetect(member)}>{detecting ? "检测中" : "检测"}</button>
            <button onClick={() => onAdmin(isAdmin ? null : member.id)}>{isAdmin ? "撤销" : "设管理"}</button>
          </>
        )}
        {member.id !== group.ownerMemberId && <button className="danger" onClick={() => onRemove(member)}>移除</button>}
      </div>
    </div>
  );
}
function Avatar({ member, responding }: { member?: Member; responding?: boolean }) {
  return (
    <span className={`avatar ${responding ? "responding" : ""}`} style={{ background: member?.avatarColor ?? "#8792a5" }}>
      {member?.displayName.slice(0, 1) ?? "?"}
    </span>
  );
}
function Status({ status }: { status: string }) { return <span className={`status ${status}`}>{({ queued: "排队中", running: "运行中", awaiting_review: "待审阅", changes_requested: "待修改", completed: "完成", failed: "失败", cancelled: "已停止", interrupted: "已中断" } as Record<string, string>)[status] ?? status}</span>; }
function ReviewBadge({ reviewStatus }: { reviewStatus: string }) { return <span className={`review-badge ${reviewStatus}`}>{({ pending: "审阅中", approved: "已通过", rejected: "被退回" } as Record<string, string>)[reviewStatus] ?? reviewStatus}</span>; }
function AuthScreen({ error, onError, onAuthed }: { error: string | null; onError: (msg: string | null) => void; onAuthed: (user: AuthUser) => void }) {
  const [mode, setMode] = useState<"login" | "register">("login");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    onError(null);
    try {
      const result = mode === "login"
        ? await api.login(username.trim(), password)
        : await api.register(username.trim(), password);
      setAuthToken(result.token);
      const user: AuthUser = {
        userId: result.user_id,
        username: result.username,
        isAdmin: Boolean(result.isAdmin ?? result.is_admin),
      };
      saveAuthUser(user);
      onAuthed(user);
    } catch (reason) {
      onError(readError(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="auth-screen">
      <section className="auth-card">
        <Brand />
        <h1>{mode === "login" ? "登录" : "注册"}</h1>
        <p className="auth-hint">多 Agent 协作工作台。使用管理员分配的账号登录后进入所属群聊。</p>
        <form className="modal-form" onSubmit={(e) => void submit(e)}>
          <label>用户名<input autoFocus value={username} onChange={(e) => setUsername(e.target.value)} required autoComplete="username" /></label>
          <label>密码<input type="password" value={password} onChange={(e) => setPassword(e.target.value)} required autoComplete={mode === "login" ? "current-password" : "new-password"} /></label>
          <button className="primary-wide" type="submit" disabled={busy}>{busy ? "请稍候…" : mode === "login" ? "进入 Workpanel" : "注册并进入"}</button>
        </form>
        <button type="button" className="auth-switch" onClick={() => { setMode(mode === "login" ? "register" : "login"); onError(null); }}>
          {mode === "login" ? "没有账号？注册" : "已有账号？登录"}
        </button>
        <ThemeSwitcher />
        {error && <div className="auth-error">{error}</div>}
      </section>
    </main>
  );
}

function Modal({ title, children, onClose }: { title: string; children: ReactNode; onClose: () => void }) { return <div className="modal-backdrop"><section className="modal"><header><h2>{title}</h2><button className="icon-button" onClick={onClose}>×</button></header>{children}</section></div>; }
function NumberSetting({ label, value, onChange, min, max }: { label: string; value: number; onChange: (value: number) => void; min: number; max: number }) { return <label>{label}<input type="number" min={min} max={max} value={value} onChange={(event) => onChange(Number(event.target.value))} /></label>; }
function readError(reason: unknown) { return typeof reason === "string" ? reason : reason instanceof Error ? reason.message : "发生了未知错误。"; }
function isUnauthorizedError(reason: unknown) {
  const message = readError(reason);
  return /401|Missing token|Invalid token|ExpiredSignature|expired|Unauthorized/i.test(message);
}
