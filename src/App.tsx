import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { FormEvent, KeyboardEvent, memo, useCallback, useEffect, useLayoutEffect, useRef, useState, type MouseEvent, type PointerEvent, type ReactNode } from "react";
import { api, getAuthToken, onUnauthorized, requiresAuth, setAuthToken } from "./api";
import {
  agentReplyDefaultOpen,
  BOTTOM_THRESHOLD_PX,
  extractReplyPreview,
  isNearBottom,
} from "./chatUi";
import { currentMentionQuery, findMentionedMemberIds } from "./mentions";
import {
  appendChannelDelta,
  hasRenderableContent,
  isLazyMessageChannel,
  parseMessageContent,
} from "./messageContent";
import { defaultModelForAdapter, modelsForAdapter, applyAgentModelsPayload } from "./agentModels";
import { canSubmitUserMember, chatbotSlotTaken, memberRosterAction, type UserAddMode } from "./memberForm";
import { InviteLanding, parseInviteTokenFromPath } from "./InviteLanding";
import { markdownToHtml } from "./markdownLite";
import { detectMemoryPressure, formatHeartbeatLabel } from "./heartbeatPolicy";
import { agentBusyLabel, queueCounts, runsForAgentActive } from "./queueCounts";
import {
  bumpUnread,
  clearUnread,
  formatUnreadBadge,
  sortGroupsForSidebar,
} from "./groupListSort";
import { isIgnorableWsKind, shouldResyncAfterWsEvent, subscribeWsLinkState } from "./realtimeWs";
import { releasingBannerText, type WsLinkState } from "./releasingState";
import type { MetricsSample } from "./types";
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
import { Divider, useAppFrame } from "./components/ui";
import { loadSendKeyMode, saveSendKeyMode, sendKeyHint, shouldSendOnKey, type SendKeyMode } from "./sendKey";
import type { ChatEvent, ExtensionStatus, Group, GroupState, Member, PresetRole, RuntimeSettings, TaskRun } from "./types";
import { MessageBubble, MemberRow, Avatar, Status, ReviewBadge, TypingIndicator, RunQueuePane, EmptyHome } from "./components/furniture";
import { PHASE_LABEL, dayLabel, time, readError } from "./components/uiShared";
import { ExperiencePanel } from "./ExperiencePanel";
import { LogsPanel } from "./LogsPanel";
import { ServerPathPicker } from "./ServerPathPicker";
import { ExtensionPanel } from "./ExtensionPanel";
import { VersionView } from "./VersionView";
import { GroupSettingsView } from "./GroupSettingsView";
import { AgentConfigView } from "./AgentConfigView";
import {
  collectExtensionTabViews,
  parseExtMainView,
} from "./extensions";
import {
  buildLiveMentionMessage,
  messageToPlainText,
  resolveLiveResponder,
} from "./liveBridge";
import {
  cancelHoldRecording,
  combineComposerAndTranscript,
  playAudioBase64,
  secureMicAvailable,
  startHoldRecording,
  stopHoldRecordingToWav,
  sttViaProxy,
  ttsPlaybackViaProxy,
} from "./liveVoice";
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
  userAddMode: UserAddMode;
  existingAuthUserId: string;
};
type Session = "checking" | "login" | "ready";
const emptyMember: NewMember = {
  kind: "agent", displayName: "", roleDescription: "", adapter: "mock", executablePath: "",
  chatbotProvider: "opencode-go", apiKey: "", model: "",
  loginUsername: "", loginPassword: "",
  userAddMode: "create", existingAuthUserId: "",
};
/** DeepSeek Harness Web UI 默认地址（`dsh web`，默认 :3080）。可手动改成实际端口。 */
const DSH_WEB_URL = "http://127.0.0.1:3080";

export function App() {
  const inviteToken =
    typeof window !== "undefined" ? parseInviteTokenFromPath(window.location.pathname) : null;
  // Web: never enter main UI until bootstrap succeeds; stale localStorage token → login.
  const [session, setSession] = useState<Session>(() => (requiresAuth ? "checking" : "ready"));
  const [groups, setGroups] = useState<Group[]>([]);
  const [groupsLoaded, setGroupsLoaded] = useState(false);
  const [onlineUserIds, setOnlineUserIds] = useState<Set<string>>(() => new Set());
  const [current, setCurrent] = useState<GroupState | null>(null);
  const [composer, setComposer] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [inviteLinkFlash, setInviteLinkFlash] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [showMembers, setShowMembers] = useState(() =>
    typeof window === "undefined" || window.matchMedia("(min-width: 1081px)").matches,
  );
  const [rightPanelTab, setRightPanelTab] = useState<"members" | "experiences" | "logs" | "queue">("members");
  const [mainView, setMainView] = useState<string>("chat");
  const [adminInAsk, setAdminInAsk] = useState(false);
  const [extensions, setExtensions] = useState<ExtensionStatus[]>([]);
  const [extTogglingId, setExtTogglingId] = useState<string | null>(null);
  const [workspacePath, setWorkspacePath] = useState("/AI/LinlisWorkPanel");
  const [createGroupKind, setCreateGroupKind] = useState<"project" | "chat">("project");
  const [showArchived, setShowArchived] = useState(false);
  const [sendKeyMode, setSendKeyMode] = useState<SendKeyMode>(() => loadSendKeyMode());
  const [authUser, setAuthUser] = useState<AuthUser | null>(() => loadAuthUser());
  const [showAddMember, setShowAddMember] = useState(false);
  const [newMember, setNewMember] = useState<NewMember>(emptyMember);
  const [joinableUsers, setJoinableUsers] = useState<{ id: string; username: string }[]>([]);
  const [ocrRunning, setOcrRunning] = useState(false);
  const [ocrPasting, setOcrPasting] = useState(false);
  const [presetRoles, setPresetRoles] = useState<PresetRole[]>([]);
  const [selectedRoles, setSelectedRoles] = useState<string[]>([]);
  const [settings, setSettings] = useState<RuntimeSettings | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [agentCfgImported, setAgentCfgImported] = useState(false);
  const [wsLink, setWsLink] = useState<{ state: WsLinkState; elapsedMs: number }>({
    state: "connected",
    elapsedMs: 0,
  });
  const [metrics, setMetrics] = useState<MetricsSample | null>(null);
  const [pageFocused, setPageFocused] = useState(
    () => typeof document === "undefined" || document.visibilityState === "visible",
  );
  const [showSidebar, setShowSidebar] = useState(false);
  const [sending, setSending] = useState(false);
  const [detecting, setDetecting] = useState<string | null>(null);
  const [mentionIndex, setMentionIndex] = useState(0);
  const messageListRef = useRef<HTMLDivElement | null>(null);
  const stickToBottom = useRef(true);
  const forceScrollGroupId = useRef<string | null>(null);
  const composerRef = useRef<HTMLTextAreaElement | null>(null);
  const [voiceHolding, setVoiceHolding] = useState(false);
  const [voiceBusy, setVoiceBusy] = useState(false);
  const [playingMessageId, setPlayingMessageId] = useState<string | null>(null);
  const playingAudioRef = useRef<HTMLAudioElement | null>(null);
  const holdActiveRef = useRef(false);
  const currentGroupIdRef = useRef<string | undefined>(undefined);
  const [showJumpBottom, setShowJumpBottom] = useState(false);
  const [visibleCount, setVisibleCount] = useState(INITIAL_VISIBLE_MESSAGES);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const [hasMoreOlder, setHasMoreOlder] = useState(false);
  const loadingOlderRef = useRef(false);
  const closeMembers = useCallback(() => setShowMembers(false), []);
  const frame = useAppFrame({ rightOpen: showMembers, onRightClose: closeMembers });
  currentGroupIdRef.current = current?.group.id;

  // P1: composer 草稿按群持久化（localStorage；草稿/几何不进 DB）
  const draftKeyFor = (groupId: string | undefined) => (groupId ? `lp.composer.${groupId}` : null);
  useEffect(() => {
    const key = draftKeyFor(current?.group.id);
    if (!key) return;
    let value = "";
    try { value = localStorage.getItem(key) ?? ""; } catch { /* ignore */ }
    setComposer(value);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [current?.group.id]);
  useEffect(() => {
    const key = draftKeyFor(current?.group.id);
    if (!key) return;
    const timer = window.setTimeout(() => {
      try { localStorage.setItem(key, composer); } catch { /* ignore */ }
    }, 350);
    return () => window.clearTimeout(timer);
  }, [composer, current?.group.id]);

  // P2: goal bar——把当前版本/Wave 上移成 chat 视图常驻条（仅项目群）
  const [goalBar, setGoalBar] = useState<{ versionName: string; waveTitle: string; done: number; total: number; status: string } | null>(null);
  useEffect(() => {
    const gid = current?.group.id;
    const isProject = current?.group.groupKind !== "chat";
    setGoalBar(null);
    if (!gid || !isProject) return;
    let cancelled = false;
    api
      .getVersionBoard(gid)
      .then((board) => {
        if (cancelled) return;
        const versions = [...(board.versions ?? [])].sort((a, b) => b.createdAt - a.createdAt);
        const version = versions[0];
        if (!version) { setGoalBar(null); return; }
        const waves = (board.waves ?? []).filter((w) => w.versionId === version.id).sort((a, b) => a.idx - b.idx);
        if (waves.length === 0) { setGoalBar(null); return; }
        const activeWave = waves.find((w) => w.status === "running" || w.status === "paused") ?? waves[waves.length - 1];
        setGoalBar({
          versionName: version.name,
          waveTitle: activeWave.title || `Wave ${activeWave.idx}`,
          done: waves.filter((w) => w.status === "done" || w.status === "skipped").length,
          total: waves.length,
          status: activeWave.status,
        });
      })
      .catch(() => { if (!cancelled) setGoalBar(null); });
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [current?.group.id, current?.group.groupKind]);

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

  const refresh = async (groupId: string | null | undefined = current?.group.id) => {
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

  useEffect(() => subscribeWsLinkState((state, elapsedMs) => setWsLink({ state, elapsedMs })), []);

  useEffect(() => {
    if (typeof document === "undefined") return;
    const onVis = () => setPageFocused(document.visibilityState === "visible");
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  }, []);

  useEffect(() => {
    if (!showSettings) {
      setMetrics(null);
      return;
    }
    let cancelled = false;
    const pull = () => {
      void api.getMetricsLatest?.().then((m) => {
        if (!cancelled) setMetrics(m);
      }).catch(() => undefined);
    };
    pull();
    const t = window.setInterval(pull, 5000);
    return () => {
      cancelled = true;
      window.clearInterval(t);
    };
  }, [showSettings]);

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
          const me = await api.verify() as { sub: string; username: string; isAdmin?: boolean; is_admin?: boolean };
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
        setGroups(sortGroupsForSidebar(boot.groups));
        setGroupsLoaded(true);
        try {
          const presence = await api.listPresence();
          if (!disposed) setOnlineUserIds(new Set(presence.onlineUserIds ?? []));
        } catch { /* optional */ }
        if (boot.groups[0]) {
          forceScrollGroupId.current = boot.groups[0].id;
          await refresh(boot.groups[0].id);
          setGroups((prev) => clearUnread(prev, boot.groups[0].id));
        } else {
          setCurrent(null);
          // Only admins may create groups when empty.
          if ((loadAuthUser()?.isAdmin ?? false)) setShowCreate(true);
        }
        try { setSettings(await api.getSettings()); } catch { /* scoped users may lack settings */ }
        try {
          const catalog = await api.getAgentModels();
          applyAgentModelsPayload(catalog);
        } catch { /* optional live catalog */ }
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
        setGroupsLoaded(true);
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
      const activeGroupId: string | null | undefined = currentGroupIdRef.current;
      if (isIgnorableWsKind(payload.kind)) return;
      if (payload.kind === "presence_snapshot" && Array.isArray(payload.onlineUserIds)) {
        setOnlineUserIds(new Set(payload.onlineUserIds.filter(Boolean) as string[]));
        return;
      }
      if (payload.kind === "presence" && payload.userId) {
        setOnlineUserIds((prev) => {
          const next = new Set(prev);
          if (payload.online) next.add(payload.userId!);
          else next.delete(payload.userId!);
          return next;
        });
        return;
      }
      if (
        payload.groupId &&
        (payload.kind === "message_created" ||
          (payload.kind === "run_status" && payload.status === "completed"))
      ) {
        setGroups((prev) => bumpUnread(prev, payload.groupId!, activeGroupId));
      }
      if (payload.kind === "orchestration_status") {
        if (activeGroupId) {
          void refresh(activeGroupId ?? undefined).catch((reason) => {
            if (isUnauthorizedError(reason)) goLogin("登录已失效，请重新登录");
            else setError(readError(reason));
          });
        }
        return;
      }
      if (shouldResyncAfterWsEvent(payload.kind)) {
        void (async () => {
          try {
            const boot = await api.bootstrap();
            setGroups(sortGroupsForSidebar(boot.groups));
            const gid = activeGroupId || boot.groups[0]?.id;
            if (gid) {
              await refresh(gid);
              try {
                const active = await api.listActiveRuns?.(gid);
                if (active) {
                  setCurrent((prev) => {
                    if (!prev || prev.group.id !== gid) return prev;
                    const byId = new Map(prev.runs.map((r) => [r.id, r]));
                    for (const r of active) byId.set(r.id, r);
                    return { ...prev, runs: [...byId.values()] };
                  });
                }
              } catch { /* optional */ }
            }
          } catch (reason) {
            if (isUnauthorizedError(reason)) goLogin("登录已失效，请重新登录");
            else setError(readError(reason));
          }
        })();
        return;
      }
      if (payload.kind === "run_heartbeat" && payload.runId) {
        setCurrent((previous) => {
          if (!previous) return previous;
          const idx = previous.runs.findIndex((r) => r.id === payload.runId);
          if (idx < 0) return previous;
          const runs = previous.runs.slice();
          runs[idx] = {
            ...runs[idx],
            status: (payload.status as TaskRun["status"]) ?? runs[idx].status,
            phaseUpdatedAt: Date.now(),
          };
          return { ...previous, runs };
        });
        return;
      }
      const inCurrent = (previous: GroupState | null) =>
        !!previous && (!payload.groupId || payload.groupId === previous.group.id
          || (!!payload.runId && previous.runs.some((r) => r.id === payload.runId)));
      if (payload.kind === "message_delta" && payload.messageId) {
        if (payload.groupId && payload.groupId !== activeGroupId) return;
        const messageId = payload.messageId;
        const channel = payload.channel ?? "final";
        const lazy = isLazyMessageChannel(channel);
        if (!lazy && !payload.delta) return;
        const delta = payload.delta ?? "";
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
          if (lazy) {
            messages[idx] = {
              ...message,
              hasThinking: channel === "thinking" || channel === "reasoning" || channel === "thought"
                ? true
                : message.hasThinking,
              hasArtifact: channel === "artifact" || channel === "tool" || channel === "tool_result"
                || channel === "command"
                ? true
                : message.hasArtifact,
              status: "streaming",
            };
          } else {
            messages[idx] = {
              ...message,
              content: appendChannelDelta(message.content, channel, delta, replace),
              status: "streaming",
            };
          }
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

  // Must stay above auth early returns — otherwise login→ready adds a hook and React #310 blanks the page.
  useEffect(() => {
    if (!showAddMember || !current || newMember.kind !== "user" || newMember.userAddMode !== "link") {
      return;
    }
    let cancelled = false;
    void api.listJoinableUsers(current.group.id).then((users) => {
      if (!cancelled) setJoinableUsers(users);
    }).catch((reason) => {
      if (!cancelled) setError(readError(reason));
    });
    return () => { cancelled = true; };
  }, [showAddMember, current?.group.id, newMember.kind, newMember.userAddMode]);

  const reloadExtensions = async (groupId?: string) => {
    const id = groupId ?? current?.group.id;
    if (!id) return;
    try {
      setExtensions(await api.listGroupExtensions(id));
    } catch {
      setExtensions([]);
    }
  };

  useEffect(() => {
    if (!current?.group.id) {
      setExtensions([]);
      return;
    }
    void reloadExtensions(current.group.id);
  }, [current?.group.id]);

  // Must stay above auth early returns — login→ready must not add hooks (React #310).
  useEffect(() => {
    const chat = current?.group.groupKind === "chat";
    if (!current || chat) {
      setAdminInAsk(false);
      return;
    }
    let cancelled = false;
    void api.getVersionBoard(current.group.id).then((board) => {
      if (!cancelled) setAdminInAsk(Boolean(board.askingVersionId));
    }).catch(() => {
      if (!cancelled) setAdminInAsk(false);
    });
    return () => { cancelled = true; };
  }, [current?.group.id, current?.group.groupKind, mainView]);

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
    setGroups((prev) => clearUnread(prev, group.id));
    void (async () => {
      try {
        await api.markGroupRead?.(group.id);
      } catch { /* getGroupState also marks read */ }
      try {
        await refresh(group.id);
      } catch (reason) {
        setError(readError(reason));
      }
    })();
  };

  if (inviteToken) {
    return (
      <InviteLanding
        token={inviteToken}
        onDone={() => {
          setError(null);
          setSession("checking");
        }}
      />
    );
  }

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
  const activeGroups = sortGroupsForSidebar(groups.filter((g) => !g.archived));
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

  /** Doubao mode B: release → STT → append draft → send (with Live @responder when present). */
  const sendVoiceTranscript = async (transcript: string) => {
    if (!current || !senderMemberId || voiceBusy || sending) return;
    const combined = combineComposerAndTranscript(composer, transcript);
    if (!combined) return;
    const responder = resolveLiveResponder(current.group, members);
    const { content, mentionIds } = buildLiveMentionMessage(combined, responder);
    const mentionSet = new Set([
      ...mentionIds,
      ...findMentionedMemberIds(content, activeMembers),
    ]);
    setComposer("");
    setSending(true);
    try {
      await api.sendMessage(current.group.id, senderMemberId, content, [...mentionSet]);
      await refresh(current.group.id);
    } catch (reason) {
      setComposer(combined);
      setError(readError(reason));
    } finally {
      setSending(false);
      composerRef.current?.focus();
    }
  };

  const onHoldTalkStart = (event: PointerEvent<HTMLButtonElement>) => {
    event.preventDefault();
    if (!liveReady || voiceBusy || sending || !current || holdActiveRef.current) return;
    if (!secureMicAvailable()) {
      setError("按住说话需要 HTTPS 或 localhost（当前无法访问麦克风）");
      return;
    }
    const target = event.currentTarget;
    target.setPointerCapture(event.pointerId);
    holdActiveRef.current = true;
    setVoiceHolding(true);
    void startHoldRecording().catch((reason) => {
      holdActiveRef.current = false;
      setVoiceHolding(false);
      setError(readError(reason));
    });
  };

  const onHoldTalkEnd = () => {
    if (!holdActiveRef.current) return;
    holdActiveRef.current = false;
    setVoiceHolding(false);
    if (!current) {
      cancelHoldRecording();
      return;
    }
    setVoiceBusy(true);
    void (async () => {
      try {
        const wav = await stopHoldRecordingToWav();
        if (!wav) return;
        const stt = await sttViaProxy(current.group.id, wav);
        if (stt.error) throw new Error(stt.error);
        const text = (stt.text ?? "").trim();
        if (!text) {
          setError("未识别到语音内容");
          return;
        }
        await sendVoiceTranscript(text);
      } catch (reason) {
        setError(readError(reason));
      } finally {
        setVoiceBusy(false);
      }
    })();
  };

  const playMessageVoice = async (messageId: string, content: string) => {
    if (!liveReady) return;
    if (playingMessageId === messageId) return;
    const plain = messageToPlainText(content);
    if (!plain) return;
    playingAudioRef.current?.pause();
    playingAudioRef.current = null;
    setPlayingMessageId(messageId);
    try {
      const tts = await ttsPlaybackViaProxy(plain);
      if (tts.error || !tts.audioBase64) throw new Error(tts.error || "TTS 无音频");
      if (tts.truncated) {
        setError(`朗读已截断至 ${tts.maxChars ?? 300} 字（purpose=playback）`);
      }
      const audio = playAudioBase64(tts.audioBase64, tts.audioContentType || "audio/mpeg");
      playingAudioRef.current = audio;
      audio.onended = () => {
        if (playingAudioRef.current === audio) {
          playingAudioRef.current = null;
          setPlayingMessageId(null);
        }
      };
      audio.onerror = () => {
        setPlayingMessageId(null);
        setError("音频播放失败");
      };
    } catch (reason) {
      setPlayingMessageId(null);
      setError(readError(reason));
    }
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
  const insertAt = () => {
    composerRef.current?.focus();
    setComposer((value) => value + "@");
    requestAnimationFrame(() => {
      const ta = composerRef.current;
      if (ta) ta.setSelectionRange(ta.value.length, ta.value.length);
    });
  };

  const toggleExtension = async (extId: string, enabled: boolean) => {
    if (!current) return;
    setExtTogglingId(extId);
    try {
      const status = await api.setExtensionEnabled(current.group.id, extId, enabled);
      setExtensions((prev) => {
        const others = prev.filter((e) => e.id !== status.id);
        return [...others, status];
      });
      const active = parseExtMainView(mainView);
      if (!enabled && active?.extId === extId) setMainView("chat");
    } catch (reason) {
      setError(readError(reason));
    } finally {
      setExtTogglingId(null);
    }
  };

  const liveExt = extensions.find((e) => e.id === "panellive") ?? null;
  const liveReady = Boolean(liveExt?.enabled && liveExt.healthy);
  const groupKind = current?.group.groupKind === "chat" ? "chat" : "project";
  const extTabViews = collectExtensionTabViews(extensions, groupKind);
  const activeExtView = (() => {
    const parsed = parseExtMainView(mainView);
    if (!parsed) return null;
    return extTabViews.find((v) => v.ext.id === parsed.extId && v.tab.id === parsed.tabId) ?? null;
  })();

  const addMember = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); if (!current) return;
    if (newMember.kind === "user" && !canSubmitUserMember(newMember.userAddMode, {
      loginUsername: newMember.loginUsername,
      loginPassword: newMember.loginPassword,
      existingAuthUserId: newMember.existingAuthUserId,
    })) {
      setError(
        newMember.userAddMode === "link"
          ? "请选择要加入的已有登录用户"
          : newMember.userAddMode === "invite"
            ? "请填写显示名称"
            : "请填写登录用户名和密码",
      );
      return;
    }
    try {
      const created = await api.addMember({
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
        loginUsername: newMember.kind === "user" && newMember.userAddMode === "create"
          ? newMember.loginUsername.trim()
          : undefined,
        loginPassword: newMember.kind === "user" && newMember.userAddMode === "create"
          ? newMember.loginPassword
          : undefined,
        existingAuthUserId: newMember.kind === "user" && newMember.userAddMode === "link"
          ? newMember.existingAuthUserId.trim()
          : undefined,
        invite: newMember.kind === "user" && newMember.userAddMode === "invite" ? true : undefined,
      });
      if (created.inviteUrl) {
        const absolute = `${window.location.origin}${created.inviteUrl}`;
        setInviteLinkFlash(absolute);
        try { await navigator.clipboard.writeText(absolute); } catch { /* ignore */ }
      } else {
        setInviteLinkFlash(null);
      }
      setNewMember(emptyMember); setShowAddMember(false); setJoinableUsers([]); await refresh();
    } catch (reason) { setError(readError(reason)); }
  };
  const removeMember = async (member: Member) => {
    if (!current) return;
    const action = memberRosterAction(member);
    if (action === "delete") {
      const label = member.invitePending ? "撤销邀请并删除" : "永久删除";
      if (!confirm(`${label} ${member.displayName}？此操作不可恢复（历史消息仍保留）。`)) return;
      try {
        await api.purgeMember(current.group.id, member.id);
        await refresh();
      } catch (reason) { setError(readError(reason)); }
      return;
    }
    if (!confirm(`移除 ${member.displayName}？成员将变为灰色，可再永久删除。`)) return;
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
  const changeRunReview = async (run: TaskRun, decision: "approved" | "rejected") => {
    try { await api.setRunReview(run.id, decision); await refresh(); } catch (reason) { setError(readError(reason)); }
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

  // P2 键盘快捷键：⌘/Ctrl+1 左栏折叠轨、⌘/Ctrl+2 成员面板、Esc 关浮层
  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent) => {
      const mod = event.ctrlKey || event.metaKey;
      if (mod && event.key === "1") { event.preventDefault(); frame.toggleLeft(); }
      else if (mod && event.key === "2") { event.preventDefault(); toggleMembers(); }
      else if (event.key === "Escape") {
        if (showCreate || showSettings || showAddMember) event.preventDefault();
        setShowCreate(false);
        setShowSettings(false);
        setShowAddMember(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [frame.toggleLeft, showCreate, showSettings, showAddMember]);

  return <main className="app-shell" ref={frame.rootRef} style={frame.rootStyle} data-left={frame.leftMode} data-right={frame.rightOpen ? "open" : "closed"}>
    {showSidebar && <div className="sidebar-backdrop" onClick={() => setShowSidebar(false)} />}
    {showMembers && <div className="members-backdrop" onClick={() => setShowMembers(false)} />}
    <aside className={`group-sidebar ${showSidebar ? "open" : ""}`}>
      <Brand />
      <div className="sidebar-heading"><span>群聊</span>{isAdmin && <button className="icon-button" onClick={() => setShowCreate(true)} aria-label="新建群聊">＋</button>}</div>
      <nav className="group-list">
        {activeGroups.map((group) => (
          <div key={group.id} className={`group-item-row ${group.id === current?.group.id ? "selected" : ""} ${(group.unreadCount ?? 0) > 0 ? "has-unread" : ""}`}>
            <button type="button" className="group-item" onClick={() => selectGroup(group)}>
              <span className="group-avatar">{group.name.slice(0, 1)}</span>
              <span className="group-name">{group.name}{group.groupKind === "chat" ? " · 聊" : ""}</span>
              {(group.unreadCount ?? 0) > 0 && (
                <em className="unread-badge" aria-label={`${group.unreadCount} 条未读`}>{formatUnreadBadge(group.unreadCount ?? 0)}</em>
              )}
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
        <button
          type="button"
          className="rail-toggle"
          title={frame.leftMode === "open" ? "折叠为控制轨（56px）· Ctrl/⌘ + 1" : "展开侧栏 · Ctrl/⌘ + 1"}
          aria-label="折叠或展开侧栏"
          onClick={() => frame.toggleLeft()}
        >
          {frame.leftMode === "open" ? "◀◀" : "▶▶"}
        </button>
        <button type="button" onClick={() => setShowSettings(true)}>运行设置</button>
        {requiresAuth && <button onClick={() => goLogin(null)}>退出登录{authUser ? `（${authUser.username}）` : ""}</button>}
      </div>
    </aside>

    <Divider {...frame.leftDivider} />

    <section className="chat-panel">
      {current ? <>
        <header className="chat-header">
          <div className="chat-header-main">
            <button className="icon-button mobile-nav" onClick={openSidebar} aria-label="群列表">☰</button>
            <div>
              <div className="group-title-row">
                <h1>{current.group.name}</h1>
                <div className="view-toggle" role="group" aria-label="主视图">
                  <button type="button" className={mainView === "chat" ? "active" : ""} onClick={() => setMainView("chat")}>聊天</button>
                  {!isChatGroup && (
                    <button type="button" className={mainView === "versions" ? "active" : ""} onClick={() => setMainView("versions")}>版本</button>
                  )}
                  <button type="button" className={mainView === "settings" ? "active" : ""} onClick={() => setMainView("settings")}>设置</button>
                  {isAdmin && (
                    <button type="button" className={mainView === "agent-config" ? "active" : ""} onClick={() => setMainView("agent-config")} title="Agent 配置：一键导入/导出、环境自检、CLI 自动安装">
                      Agent 配置{agentCfgImported ? "✓" : ""}
                    </button>
                  )}
                  {extTabViews.map(({ ext, tab, viewKey }) => {
                    const ready = Boolean(ext.enabled && ext.healthy);
                    return (
                      <button
                        key={viewKey}
                        type="button"
                        className={mainView === viewKey ? "active" : ""}
                        disabled={!ext.enabled}
                        title={
                          !ext.enabled
                            ? `请先在运行设置中开启 ${ext.name}`
                            : ready
                              ? tab.title
                              : `${ext.name} 未就绪`
                        }
                        onClick={() => setMainView(viewKey)}
                      >
                        {tab.title}{ext.enabled && !ready ? "·离线" : ""}
                      </button>
                    );
                  })}
                </div>
              </div>
              <p>
                {activeMembers.length} 名成员 · {isChatGroup ? "聊天群" : "项目群"}
                {!isChatGroup && current.group.workspacePath
                  ? ` · ${current.group.workspacePath}`
                  : ""}
                {" · "}
                <button type="button" className="linkish" onClick={() => setMainView("settings")}>
                  公告 / 工作目录在「设置」
                </button>
                {extTabViews.some((v) => v.ext.enabled)
                  ? ` · Extend ${extTabViews.filter((v) => v.ext.enabled && v.ext.healthy).length}/${extTabViews.filter((v) => v.ext.enabled).length} 就绪`
                  : ""}
              </p>
            </div>
          </div>
          <button
            type="button"
            className="members-toggle"
            onClick={toggleMembers}
            aria-label="成员 / Agent 面板"
            aria-pressed={showMembers}
            title="成员 / Agent 面板（Ctrl/⌘ + 2）"
          >
            {showMembers ? "▤" : "▥"} 成员
          </button>
        </header>
        {goalBar && (
          <div
            className="goal-bar"
            data-status={goalBar.status}
            onClick={() => setMainView("versions")}
            role="button"
            tabIndex={0}
            onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setMainView("versions"); } }}
            title="查看版本 / Wave"
          >
            <span className="g-flag">{goalBar.versionName}</span>
            <span className="g-wave">{goalBar.waveTitle}</span>
            <span className="progress"><i style={{ width: `${goalBar.total ? Math.round((goalBar.done / goalBar.total) * 100) : 0}%` }} /></span>
            <span className="g-act">{goalBar.done}/{goalBar.total} 完成</span>
          </div>
        )}
        {activeExtView ? (
          <ExtensionPanel
            extension={activeExtView.ext}
            tab={activeExtView.tab}
            group={current?.group ?? null}
            members={members}
            messages={current?.messages ?? []}
            senderMemberId={senderMemberId}
            onOpenSettings={() => setShowSettings(true)}
            onError={(msg) => setError(msg)}
          />
        ) : !isChatGroup && mainView === "versions" ? (
          <VersionView
            group={current.group}
            members={members}
            senderMemberId={senderMemberId}
            canManage={isAdmin || Boolean(senderMemberId && current.group.ownerMemberId === senderMemberId)}
            onError={(msg) => setError(msg)}
            onGotoChat={() => setMainView("chat")}
          />
        ) : mainView === "dsh" ? (
            <DSHView onClose={() => setMainView("chat")} />
          ) : mainView === "settings" ? (
          <GroupSettingsView
            group={current.group}
            members={members}
            canManage={isAdmin || Boolean(senderMemberId && current.group.ownerMemberId === senderMemberId)}
            onGroupPatch={(g) => {
              setCurrent((prev) => (prev ? { ...prev, group: { ...prev.group, ...g } } : prev));
              setGroups((prev) => prev.map((x) => (x.id === g.id ? { ...x, ...g } : x)));
            }}
            onMemberPatch={(m) => {
              setCurrent((prev) => {
                if (!prev) return prev;
                return {
                  ...prev,
                  members: prev.members.map((x) => (x.id === m.id ? { ...x, ...m } : x)),
                };
              });
            }}
            onError={(msg) => setError(msg)}
          />
        ) : mainView === "agent-config" ? (
          <AgentConfigView
            onError={(msg) => setError(msg)}
            onStatusChange={setAgentCfgImported}
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
                <MessageBubble
                  message={message}
                  members={members}
                  runs={current.runs}
                  viewerMemberId={senderMemberId}
                  onRun={changeRun}
                  voiceUxEnabled={liveReady}
                  playingMessageId={playingMessageId}
                  onPlayVoice={playMessageVoice}
                />
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
          <div className="composer-tools">
            <button type="button" className="tool-btn" title="提及成员（输入 @ 亦可）" onClick={insertAt}>@</button>
            <span className="composer-hint-text">
              <kbd>@</kbd> 提及成员 · <kbd>Enter</kbd> 发送 · 草稿自动保存（本机）
            </span>
          </div>
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
              {liveReady && (
                <button
                  type="button"
                  className={`hold-talk-button${voiceHolding ? " recording" : ""}`}
                  disabled={voiceBusy || sending}
                  title={secureMicAvailable() ? "按住说话，松手发送" : "需要 HTTPS 或 localhost 才能使用麦克风"}
                  onPointerDown={onHoldTalkStart}
                  onPointerUp={onHoldTalkEnd}
                  onPointerCancel={onHoldTalkEnd}
                  onContextMenu={(e) => e.preventDefault()}
                >
                  {voiceBusy ? "识别中…" : voiceHolding ? "松开发送" : "按住说话"}
                </button>
              )}
              <button className="send-button" disabled={!composer.trim() || sending} onClick={() => void send()}>{sending ? "发送中" : "发送"}</button>
            </span>
          </div>
        </footer>
        </>}
      </> : groups.length === 0 && groupsLoaded ? (
        <EmptyHome canCreate={isAdmin} onCreate={() => setShowCreate(true)} />
      ) : <div className="loading">正在打开本地群聊…</div>}
    </section>

    <Divider {...frame.rightDivider} />

    {current && showMembers && <aside className="member-panel">
      <header>
        <div className="pm-tab-bar">
          <button className={`pm-tab-btn ${rightPanelTab === "members" ? "active" : ""}`} onClick={() => setRightPanelTab("members")}>群成员</button>
          <button className={`pm-tab-btn ${rightPanelTab === "queue" ? "active" : ""}`} onClick={() => setRightPanelTab("queue")}>队列</button>
          {!isChatGroup && isAdmin && <button className={`pm-tab-btn ${mainView === "versions" ? "active" : ""}`} onClick={() => { setMainView("versions"); }}>版本管理</button>}
          <button className={`pm-tab-btn ${mainView === "settings" ? "active" : ""}`} onClick={() => { setMainView("settings"); setShowMembers(false); }}>群设置</button>
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
        <div className="member-list">{members.map((member) => (
          <MemberRow
            key={member.id}
            member={member}
            group={current.group}
            askMode={adminInAsk && member.id === current.group.adminMemberId}
            runs={current.runs}
            online={member.kind === "user" && !!member.authUserId && onlineUserIds.has(member.authUserId)}
            detecting={detecting === member.id}
            onAdmin={setAdmin}
            onRemove={removeMember}
            onDetect={detect}
            onModel={(m, model) => void changeMemberModel(m, model)}
            onCancelRun={(run) => void changeRun(run, "cancel")}
              onOpenDsh={() => setMainView("dsh")}
          />
        ))}</div>
        {inviteLinkFlash && (
          <div className="invite-link-flash">
            <p>邀请链接已生成（已尝试复制到剪贴板，24 小时有效）：</p>
            <code>{inviteLinkFlash}</code>
            <button type="button" onClick={() => void navigator.clipboard.writeText(inviteLinkFlash)}>再复制</button>
            <button type="button" onClick={() => setInviteLinkFlash(null)}>关闭</button>
          </div>
        )}
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
            <select
              value={newMember.userAddMode}
              onChange={(event) => setNewMember((value) => ({
                ...value,
                userAddMode: event.target.value as UserAddMode,
                existingAuthUserId: "",
              }))}
            >
              <option value="create">创建新账号</option>
              <option value="link">加入已有账号</option>
              <option value="invite">邀请链接（待接受）</option>
            </select>
            {newMember.userAddMode === "create" ? <>
              <input value={newMember.loginUsername} onChange={(event) => setNewMember((value) => ({ ...value, loginUsername: event.target.value }))} placeholder="登录用户名" required autoComplete="off" />
              <input type="password" value={newMember.loginPassword} onChange={(event) => setNewMember((value) => ({ ...value, loginPassword: event.target.value }))} placeholder="登录密码" required autoComplete="new-password" />
              <p className="form-hint">对方用该用户名/密码登录后，只能看到并进入本群对话。若用户名已存在，请改选「加入已有账号」。</p>
            </> : newMember.userAddMode === "invite" ? (
              <p className="form-hint">创建占位成员并生成 24 小时邀请链接；对方登录/注册后自动入群。接受前成员显示「链接中」。</p>
            ) : <>
              <select
                value={newMember.existingAuthUserId}
                onChange={(event) => {
                  const id = event.target.value;
                  const picked = joinableUsers.find((u) => u.id === id);
                  setNewMember((value) => ({
                    ...value,
                    existingAuthUserId: id,
                    displayName: value.displayName.trim() || picked?.username || value.displayName,
                  }));
                }}
                required
              >
                <option value="">选择已有登录用户…</option>
                {joinableUsers.map((u) => (
                  <option key={u.id} value={u.id}>{u.username}</option>
                ))}
              </select>
              <p className="form-hint">将已有登录用户拉入本群（无需再设密码）；列表不含已在本群的用户。</p>
            </>}
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
                <option value="dsh">DeepSeek Harness（dsh）</option>
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
      : rightPanelTab === "queue" ? <RunQueuePane runs={current.runs} members={members} onCancel={(run) => void changeRun(run, "cancel")} onReview={(run, decision) => void changeRunReview(run, decision)} />
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
    {showSettings && (
      <Modal title="运行设置" onClose={() => setShowSettings(false)}>
        <div className="modal-form settings-modal">
          <ThemeSwitcher />
          {isAdmin && current && (
            <div className="extension-settings">
              <h3 className="settings-section-title">Extend</h3>
              {extensions.length === 0 ? (
                <p className="form-hint">未发现扩展（检查 LINLIS_EXTENSION_ROOTS / 清单文件）。</p>
              ) : (
                extensions.map((ext) => (
                  <div key={ext.id} className="extension-settings-row">
                    <label className="settings-check">
                      <input
                        type="checkbox"
                        checked={Boolean(ext.enabled)}
                        disabled={extTogglingId === ext.id}
                        onChange={(e) => void toggleExtension(ext.id, e.target.checked)}
                      />
                      {ext.name}
                      <span className="form-hint" style={{ marginLeft: 8 }}>
                        {ext.version} · {ext.healthy ? "health ok" : "health down"}
                        {ext.healthDetail ? ` · ${ext.healthDetail}` : ""}
                      </span>
                    </label>
                  </div>
                ))
              )}
              <p className="form-hint">启用后顶栏出现 manifest 页签；媒体面由扩展自理，A2A 仅文本 skills（禁 PCM）。</p>
            </div>
          )}
          <div className="extension-settings">
            <h3 className="settings-section-title">进程指标（主进程）</h3>
            <p className="form-hint">
              {metrics
                ? `CPU ${metrics.cpuPct.toFixed(1)}% · RSS ${metrics.rssMib.toFixed(1)} MiB · 采样 ${new Date(metrics.ts).toLocaleTimeString()}`
                : "打开设置后每 5s 拉取 /api/metrics/latest（后台仍 20s 落库）"}
            </p>
          </div>
          {isAdmin && settings ? (
            <form className="modal-form" onSubmit={saveSettings}>
              <NumberSetting label="每群并发任务" value={settings.maxConcurrentRuns} onChange={(value) => setSettings({ ...settings, maxConcurrentRuns: value })} min={1} max={8} />
              <NumberSetting label="任务超时（秒）" value={settings.runTimeoutSeconds} onChange={(value) => setSettings({ ...settings, runTimeoutSeconds: value })} min={30} max={7200} />
              <NumberSetting label="工作群上下文消息数" value={settings.contextMessageLimit} onChange={(value) => setSettings({ ...settings, contextMessageLimit: value })} min={5} max={200} />
              <NumberSetting label="聊天群/机器人上下文" value={settings.chatContextMessageLimit ?? 12} onChange={(value) => setSettings({ ...settings, chatContextMessageLimit: value })} min={5} max={40} />
              <p className="form-hint">聊天群/chatbot 默认保留最近 12 条原文；超出时折叠进历史摘要后再累加。无向量长期记忆。</p>
              <NumberSetting label="管理员最大派生层级" value={settings.maxDelegationDepth} onChange={(value) => setSettings({ ...settings, maxDelegationDepth: value })} min={0} max={4} />
              <h3 className="settings-section-title">心跳</h3>
              <label className="settings-check">
                <input
                  type="checkbox"
                  checked={settings.heartbeatAuto !== false}
                  onChange={(e) => setSettings({ ...settings, heartbeatAuto: e.target.checked })}
                />
                Auto（聚焦/后台动态频率；不做 100ms HTTP 轮询）
              </label>
              <NumberSetting
                label="聚焦心跳（秒）"
                value={settings.heartbeatFocusSeconds ?? 1}
                onChange={(value) => setSettings({ ...settings, heartbeatFocusSeconds: value })}
                min={1}
                max={30}
              />
              <NumberSetting
                label="非聚焦心跳（秒）"
                value={settings.heartbeatBackgroundSeconds ?? 5}
                onChange={(value) => setSettings({ ...settings, heartbeatBackgroundSeconds: value })}
                min={1}
                max={60}
              />
              <p className="form-hint">
                {formatHeartbeatLabel({
                  focused: pageFocused,
                  settings: {
                    heartbeatAuto: settings.heartbeatAuto !== false,
                    heartbeatFocusSeconds: settings.heartbeatFocusSeconds ?? 1,
                    heartbeatBackgroundSeconds: settings.heartbeatBackgroundSeconds ?? 5,
                  },
                  memoryPressure: detectMemoryPressure(
                    typeof navigator !== "undefined"
                      ? (navigator as Navigator & { deviceMemory?: number }).deviceMemory
                      : undefined,
                  ),
                })}
              </p>
              <button className="primary-wide" type="submit">保存设置</button>
            </form>
          ) : (
            <p className="form-hint">主题已即时生效。运行参数仅管理员可改。</p>
          )}
        </div>
      </Modal>
    )}
    {releasingBannerText(wsLink.state, wsLink.elapsedMs) && (
      <div className="error-toast" style={{ bottom: error ? 64 : 16 }}>
        <span>{releasingBannerText(wsLink.state, wsLink.elapsedMs)}</span>
      </div>
    )}
    {error && <div className="error-toast"><span>{error}</span><button onClick={() => setError(null)}>×</button></div>}
  </main>;
}

function DSHView({ onClose }: { onClose?: () => void }) {
  return (
    <div className="live-panel dsh-view">
      <div className="dsh-view-head">
        <p className="live-panel-hint">
          已嵌入 DeepSeek Harness Web 界面（{DSH_WEB_URL}）。
          若未启动，请在服务器上运行 <code>dsh web</code>（默认 :3080）；面板直接嵌入本机该端口。
        </p>
        {onClose && (
          <button type="button" className="pm-btn sm" onClick={onClose}>
            返回聊天
          </button>
        )}
      </div>
      <iframe
        title="DeepSeek Harness Web"
        className="live-frame"
        src={DSH_WEB_URL}
        allow="microphone; autoplay"
      />
    </div>
  );
}


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
        {error && <div className="auth-error">{error}</div>}
      </section>
    </main>
  );
}

function Modal({ title, children, onClose }: { title: string; children: ReactNode; onClose: () => void }) { return <div className="modal-backdrop"><section className="modal"><header><h2>{title}</h2><button className="icon-button" onClick={onClose}>×</button></header>{children}</section></div>; }
function NumberSetting({ label, value, onChange, min, max }: { label: string; value: number; onChange: (value: number) => void; min: number; max: number }) { return <label>{label}<input type="number" min={min} max={max} value={value} onChange={(event) => onChange(Number(event.target.value))} /></label>; }
function isUnauthorizedError(reason: unknown) {
  const message = readError(reason);
  return /401|Missing token|Invalid token|ExpiredSignature|expired|Unauthorized/i.test(message);
}
