import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { api } from "./api";
import type { Group, Member, ProjectVersion, VersionBoard, Wave } from "./types";

const PHASE_LABEL: Record<string, string> = {
  assign: "原始需求分配",
  clarify: "需求澄清",
  design: "需求设计",
  develop: "迭代开发",
  verify: "测试灰度验收",
  summary: "总结",
};

interface Props {
  group: Group;
  members: Member[];
  senderMemberId: string | null;
  canManage: boolean;
  onError: (msg: string) => void;
  onGotoChat?: () => void;
}

export function VersionView({ group, members, senderMemberId, canManage, onError, onGotoChat }: Props) {
  const [board, setBoard] = useState<VersionBoard | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [draft, setDraft] = useState({ name: "", what: "", who: "", how: "", oneLiner: "" });

  const refresh = useCallback(async () => {
    try {
      const next = await api.getVersionBoard(group.id);
      setBoard(next);
      const latest = next.versions[0]?.id ?? next.git.tags[0]?.name ?? null;
      setExpandedId((prev) => prev ?? latest);
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [group.id, onError]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const wavesByVersion = useMemo(() => {
    const map = new Map<string, Wave[]>();
    for (const w of board?.waves ?? []) {
      const list = map.get(w.versionId) ?? [];
      list.push(w);
      map.set(w.versionId, list);
    }
    return map;
  }, [board?.waves]);

  const run = async (key: string, fn: () => Promise<void>) => {
    setBusy(key);
    try {
      await fn();
      await refresh();
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  };

  const onCreate = (mode: "create" | "import") =>
    run(`create-${mode}`, async () => {
      if (!senderMemberId) throw new Error("无发送者成员身份");
      await api.createProjectVersion({
        groupId: group.id,
        mode,
        name: draft.name || undefined,
        what: draft.what || undefined,
        who: draft.who || undefined,
        how: draft.how || undefined,
        oneLiner: draft.oneLiner || undefined,
        requesterMemberId: senderMemberId,
      });
      setDraft({ name: "", what: "", who: "", how: "", oneLiner: "" });
    });

  const saveRoadmap = (v: ProjectVersion, event: FormEvent) => {
    event.preventDefault();
    const fd = new FormData(event.target as HTMLFormElement);
    void run(`road-${v.id}`, async () => {
      await api.updateVersionRoadmap(v.id, {
        name: String(fd.get("name") || v.name),
        what: String(fd.get("what") || ""),
        who: String(fd.get("who") || ""),
        how: String(fd.get("how") || ""),
        oneLiner: String(fd.get("oneLiner") || ""),
        requesterMemberId: senderMemberId ?? undefined,
      });
    });
  };

  if (loading) return <div className="version-view loading">加载版本页…</div>;
  if (!board) return <div className="version-view empty-chat">无法加载版本板</div>;

  const admin = members.find((m) => m.id === board.adminMemberId);

  return (
    <div className="version-view">
      <header className="version-view-header">
        <div>
          <h2>版本</h2>
          <p>
            {board.git.isGitRepo
              ? `Git · HEAD ${(board.git.headSha || "").slice(0, 8)}${board.git.finishLastRound ? " · finish-last-round" : ""}`
              : "工作区不是 git 仓库（仍可新建草稿版本）"}
            {board.askingVersionId ? " · Ask 进行中" : ""}
            {admin ? ` · 管理员 ${admin.displayName}` : " · 未设管理员"}
          </p>
          {(board.workspacePath || group.workspacePath) && (
            <p className="version-workspace-hint">
              Git 来自工作区 <code>{board.workspacePath || group.workspacePath}</code>
              {" · "}版本记录按本群隔离
              {(board.workspaceSharedWith?.length ?? 0) > 0
                ? ` · 与其他群共享该路径：${board.workspaceSharedWith!.join("、")}`
                : ""}
            </p>
          )}
        </div>
        {canManage && (
          <div className="version-actions">
            <button type="button" className="pm-btn sm" disabled={!!busy || !board.adminMemberId} onClick={() => void onCreate("import")}>
              导入版本
            </button>
            <button type="button" className="pm-btn primary sm" disabled={!!busy || !board.adminMemberId} onClick={() => void onCreate("create")}>
              新建版本
            </button>
          </div>
        )}
      </header>

      {board.git.finishLastRound && (
        <div className="version-banner">本轮已完成（HEAD 停在最新 Tag）。可新建版本开启下一轮。</div>
      )}
      {(board.workspaceSharedWith?.length ?? 0) > 0 && (
        <div className="version-banner warn">
          多个项目群指向同一工作区时，Git Tag 时间线会看起来一样；上方「版本」列表仍只属于本群。
        </div>
      )}
      {!board.adminMemberId && (
        <div className="version-banner warn">请先在成员面板设置管理员 Agent，否则无法 Ask / 执行 Wave。</div>
      )}
      {board.git.error && <div className="version-banner warn">{board.git.error}</div>}

      {canManage && (
        <details className="version-draft">
          <summary>新建/导入时可选填 Roadmap 草稿</summary>
          <div className="version-draft-grid">
            <input placeholder="版本名（如 v1.3.0）" value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} />
            <input placeholder="一句话需求" value={draft.oneLiner} onChange={(e) => setDraft({ ...draft, oneLiner: e.target.value })} />
            <textarea placeholder="What" value={draft.what} onChange={(e) => setDraft({ ...draft, what: e.target.value })} />
            <textarea placeholder="Who" value={draft.who} onChange={(e) => setDraft({ ...draft, who: e.target.value })} />
            <textarea placeholder="How" value={draft.how} onChange={(e) => setDraft({ ...draft, how: e.target.value })} />
          </div>
        </details>
      )}

      <div className="version-timeline">
        {board.versions.length === 0 && board.git.tags.map((tag) => (
          <article key={tag.name} className={`version-card ${expandedId === tag.name ? "open" : ""}`}>
            <button type="button" className="version-card-head" onClick={() => setExpandedId(expandedId === tag.name ? null : tag.name)}>
              <strong>{tag.name}</strong>
              <span>{tag.isVirtual ? "虚拟" : "Tag"} · {tag.sha.slice(0, 8)}</span>
            </button>
            {expandedId === tag.name && (
              <div className="version-card-body">
                <p>{tag.subject || "尚未入库为项目版本。请「新建」或「导入」以启动 Roadmap。"}</p>
              </div>
            )}
          </article>
        ))}

        {board.versions.map((v, index) => {
          const open = expandedId === v.id || (expandedId == null && index === 0);
          const waves = wavesByVersion.get(v.id) ?? [];
          return (
            <article key={v.id} className={`version-card ${open ? "open" : ""} status-${v.status}`}>
              <button type="button" className="version-card-head" onClick={() => setExpandedId(open && expandedId === v.id ? null : v.id)}>
                <strong>{v.name}</strong>
                <span>{v.status} · {v.kind}{v.gitTag ? ` · ${v.gitTag}` : ""}</span>
              </button>
              {open && (
                <div className="version-card-body">
                  <form className="version-roadmap" onSubmit={(e) => saveRoadmap(v, e)}>
                    <label>名称<input name="name" defaultValue={v.name} disabled={!canManage} /></label>
                    <label>一句话<input name="oneLiner" defaultValue={v.oneLiner} disabled={!canManage} /></label>
                    <label>What<textarea name="what" defaultValue={v.what} disabled={!canManage} rows={3} /></label>
                    <label>Who<textarea name="who" defaultValue={v.who} disabled={!canManage} rows={2} /></label>
                    <label>How<textarea name="how" defaultValue={v.how} disabled={!canManage} rows={2} /></label>
                    {canManage && (
                      <div className="version-actions">
                        <button type="submit" className="pm-btn sm" disabled={!!busy}>保存 Roadmap</button>
                        {(v.status === "planning" || v.status === "asking") && (
                          <button
                            type="button"
                            className="pm-btn primary sm"
                            disabled={!!busy || !senderMemberId}
                            onClick={() => void run(`ask-${v.id}`, async () => {
                              if (!senderMemberId) throw new Error("无发送者");
                              await api.startVersionAsk(v.id, senderMemberId);
                              onGotoChat?.();
                            })}
                          >
                            {v.status === "asking" ? "继续 Ask" : "开始 Ask"}
                          </button>
                        )}
                        {v.status === "asking" && (
                          <>
                            <button type="button" className="pm-btn sm" disabled={!!busy} onClick={() => void run(`cancel-${v.id}`, () => api.cancelVersionAsk(v.id).then(() => undefined))}>
                              退出 Ask
                            </button>
                            <button
                              type="button"
                              className="pm-btn primary sm"
                              disabled={!!busy}
                              onClick={() => void run(`waves-${v.id}`, async () => {
                                await api.approveVersionWaves(v.id);
                              })}
                            >
                              确认默认 Waves
                            </button>
                          </>
                        )}
                        {(v.status === "ready" || v.status === "wave_running" || v.status === "paused") && (
                          <>
                            <button type="button" className="pm-btn primary sm" disabled={!!busy || !senderMemberId} onClick={() => void run(`vplay-${v.id}`, async () => {
                              if (!senderMemberId) throw new Error("无发送者");
                              await api.playVersion(v.id, senderMemberId);
                              onGotoChat?.();
                            })}>▶ 播放 Roadmap</button>
                            <button type="button" className="pm-btn sm" disabled={!!busy} onClick={() => void run(`vpause-${v.id}`, () => api.pauseVersion(v.id).then(() => undefined))}>⏸ 暂停</button>
                          </>
                        )}
                        {v.status === "awaiting_release" && (
                          <button type="button" className="pm-btn primary sm" disabled={!!busy} onClick={() => void run(`rel-${v.id}`, () => api.releaseVersion(v.id, v.name).then(() => undefined))}>
                            标记已发布（SIT 后）
                          </button>
                        )}
                      </div>
                    )}
                  </form>

                  {waves.length > 0 && (
                    <div className="wave-list">
                      <h3>Waves</h3>
                      {waves.map((w) => (
                        <div key={w.id} className={`wave-row ${w.playState}`}>
                          <div>
                            <strong>W{w.idx}. {w.title}</strong>
                            <span>{w.status} · {PHASE_LABEL[w.phase] ?? w.phase}</span>
                          </div>
                          {canManage && (
                            <div className="version-actions">
                              {w.playState !== "playing" ? (
                                <button type="button" className="pm-btn sm" disabled={!!busy || !senderMemberId} title="播放" onClick={() => void run(`wplay-${w.id}`, async () => {
                                  if (!senderMemberId) throw new Error("无发送者");
                                  await api.playWave(w.id, senderMemberId);
                                  onGotoChat?.();
                                })}>▶</button>
                              ) : (
                                <button type="button" className="pm-btn sm" disabled={!!busy} onClick={() => void run(`wpause-${w.id}`, () => api.pauseWave(w.id).then(() => undefined))}>⏸</button>
                              )}
                              <button type="button" className="pm-btn sm" disabled={!!busy} onClick={() => void run(`wadv-${w.id}`, () => api.advanceWave(w.id).then(() => undefined))}>下一阶段</button>
                            </div>
                          )}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </article>
          );
        })}
      </div>
    </div>
  );
}
