//! 版本视图（设计 P06）：左侧版本列表 + 右侧详情。
//!
//! 只重排布局，不改变接口调用：Ask / Wave / Release 仍按服务端状态提供。

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";

import { api } from "../api";
import type { Group, Member, ProjectVersion, VersionBoard, Wave } from "../types";

const PHASE_LABEL: Record<string, string> = {
  assign: "原始需求分配",
  clarify: "需求澄清",
  design: "需求设计",
  develop: "迭代开发",
  verify: "测试灰度验收",
  summary: "总结",
};

const STATUS_LABEL: Record<string, string> = {
  draft: "草稿",
  planning: "规划",
  asking: "Ask 中",
  ready: "待执行",
  wave_running: "执行中",
  paused: "已暂停",
  awaiting_release: "待发布",
  released: "已发布",
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
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draft, setDraft] = useState({ name: "", what: "", who: "", how: "", oneLiner: "" });

  const refresh = useCallback(async () => {
    try {
      const next = await api.getVersionBoard(group.id);
      setBoard(next);
      setSelectedId((previous) => {
        if (previous && next.versions.some((version) => version.id === previous)) return previous;
        return next.versions[0]?.id ?? null;
      });
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
    for (const wave of board?.waves ?? []) {
      const list = map.get(wave.versionId) ?? [];
      list.push(wave);
      map.set(wave.versionId, list);
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

  const saveRoadmap = (version: ProjectVersion, event: FormEvent) => {
    event.preventDefault();
    const form = new FormData(event.target as HTMLFormElement);
    void run(`road-${version.id}`, async () => {
      await api.updateVersionRoadmap(version.id, {
        name: String(form.get("name") || version.name),
        what: String(form.get("what") || ""),
        who: String(form.get("who") || ""),
        how: String(form.get("how") || ""),
        oneLiner: String(form.get("oneLiner") || ""),
        requesterMemberId: senderMemberId ?? undefined,
      });
    });
  };

  if (loading) return <div className="version-view loading">加载版本页…</div>;
  if (!board) return <div className="version-view empty-chat">无法加载版本板</div>;

  const admin = members.find((member) => member.id === board.adminMemberId);
  const selected = board.versions.find((version) => version.id === selectedId) ?? null;
  const waves = selected ? (wavesByVersion.get(selected.id) ?? []) : [];

  return (
    <div className="version-view">
      <header className="version-head">
        <div>
          <p className="version-git">
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
            <button
              type="button"
              className="pm-btn sm"
              disabled={!!busy || !board.adminMemberId}
              title={board.adminMemberId ? undefined : "请先在成员面板指定管理员 Agent"}
              onClick={() => void onCreate("import")}
            >
              导入版本
            </button>
            <button
              type="button"
              className="pm-btn primary sm"
              disabled={!!busy || !board.adminMemberId}
              title={board.adminMemberId ? undefined : "请先在成员面板指定管理员 Agent"}
              onClick={() => void onCreate("create")}
            >
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
          多个项目群指向同一工作区时，Git Tag 时间线会看起来一样；左侧版本列表仍只属于本群。
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

      <div className="version-pane">
        <aside className="version-list" aria-label="版本列表">
          <h3>版本</h3>
          {board.versions.length === 0 ? (
            <p className="version-list-empty">
              {board.git.tags.length > 0 ? "暂无入库版本；可导入已有 Tag。" : "还没有版本。"}
            </p>
          ) : (
            <ul>
              {board.versions.map((version) => (
                <li key={version.id}>
                  <button
                    type="button"
                    className={`version-item${version.id === selectedId ? " is-active" : ""}`}
                    aria-current={version.id === selectedId}
                    onClick={() => setSelectedId(version.id)}
                  >
                    <span className="version-item-name">{version.name}</span>
                    <span className="version-item-meta">
                      <span>{STATUS_LABEL[version.status] ?? version.status}</span>
                      <span>{version.kind}</span>
                      {version.gitTag && <code>{version.gitTag}</code>}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </aside>

        <section className="version-detail" aria-label="版本详情">
          {!selected ? (
            <div className="version-empty">
              <p>选择左侧版本查看 Roadmap、Wave 与发布状态。</p>
              {board.git.tags.length > 0 && (
                <p className="version-workspace-hint">
                  检测到 Tag：{board.git.tags.slice(0, 6).map((tag) => tag.name).join("、")}
                </p>
              )}
            </div>
          ) : (
            <>
              <header className="version-detail-head">
                <div>
                  <h2>{selected.name}</h2>
                  <p className="version-detail-meta">
                    <span>{STATUS_LABEL[selected.status] ?? selected.status}</span>
                    <span>{selected.kind}</span>
                    {selected.gitTag && <code>{selected.gitTag}</code>}
                  </p>
                  {selected.oneLiner && <p className="version-oneliner">{selected.oneLiner}</p>}
                </div>
              </header>

              <form className="version-roadmap" onSubmit={(e) => saveRoadmap(selected, e)}>
                <label>名称<input name="name" defaultValue={selected.name} disabled={!canManage} /></label>
                <label>一句话<input name="oneLiner" defaultValue={selected.oneLiner} disabled={!canManage} /></label>
                <label>What<textarea name="what" defaultValue={selected.what} disabled={!canManage} rows={3} /></label>
                <label>Who<textarea name="who" defaultValue={selected.who} disabled={!canManage} rows={2} /></label>
                <label>How<textarea name="how" defaultValue={selected.how} disabled={!canManage} rows={2} /></label>
                {canManage && (
                  <div className="version-actions">
                    <button type="submit" className="pm-btn sm" disabled={!!busy}>保存 Roadmap</button>
                    {(selected.status === "planning" || selected.status === "asking") && (
                      <button
                        type="button"
                        className="pm-btn primary sm"
                        disabled={!!busy || !senderMemberId}
                        onClick={() => void run(`ask-${selected.id}`, async () => {
                          if (!senderMemberId) throw new Error("无发送者");
                          await api.startVersionAsk(selected.id, senderMemberId);
                          onGotoChat?.();
                        })}
                      >
                        {selected.status === "asking" ? "继续 Ask" : "开始 Ask"}
                      </button>
                    )}
                    {selected.status === "asking" && (
                      <>
                        <button type="button" className="pm-btn sm" disabled={!!busy} onClick={() => void run(`cancel-${selected.id}`, () => api.cancelVersionAsk(selected.id).then(() => undefined))}>
                          退出 Ask
                        </button>
                        <button
                          type="button"
                          className="pm-btn primary sm"
                          disabled={!!busy}
                          onClick={() => void run(`waves-${selected.id}`, async () => {
                            await api.approveVersionWaves(selected.id);
                          })}
                        >
                          确认默认 Waves
                        </button>
                      </>
                    )}
                    {(selected.status === "ready" || selected.status === "wave_running" || selected.status === "paused") && (
                      <>
                        <button type="button" className="pm-btn primary sm" disabled={!!busy || !senderMemberId} onClick={() => void run(`vplay-${selected.id}`, async () => {
                          if (!senderMemberId) throw new Error("无发送者");
                          await api.playVersion(selected.id, senderMemberId);
                          onGotoChat?.();
                        })}>▶ 播放 Roadmap</button>
                        <button type="button" className="pm-btn sm" disabled={!!busy} onClick={() => void run(`vpause-${selected.id}`, () => api.pauseVersion(selected.id).then(() => undefined))}>⏸ 暂停</button>
                      </>
                    )}
                    {selected.status === "awaiting_release" && (
                      <button type="button" className="pm-btn primary sm" disabled={!!busy} onClick={() => void run(`rel-${selected.id}`, () => api.releaseVersion(selected.id, selected.name).then(() => undefined))}>
                        标记已发布（SIT 后）
                      </button>
                    )}
                  </div>
                )}
              </form>

              <div className="wave-list">
                <h3>Waves</h3>
                {waves.length === 0 ? (
                  <p className="version-workspace-hint">还没有 Wave。Ask 完成后可确认默认 Waves。</p>
                ) : (
                  waves.map((wave) => (
                    <div key={wave.id} className={`wave-row ${wave.playState}`}>
                      <div>
                        <strong>W{wave.idx}. {wave.title}</strong>
                        <span>{wave.status} · {PHASE_LABEL[wave.phase] ?? wave.phase}</span>
                        {wave.summary && <p>{wave.summary}</p>}
                      </div>
                      {canManage && (
                        <div className="version-actions">
                          {wave.playState !== "playing" ? (
                            <button type="button" className="pm-btn sm" disabled={!!busy || !senderMemberId} title="播放" onClick={() => void run(`wplay-${wave.id}`, async () => {
                              if (!senderMemberId) throw new Error("无发送者");
                              await api.playWave(wave.id, senderMemberId);
                              onGotoChat?.();
                            })}>▶</button>
                          ) : (
                            <button type="button" className="pm-btn sm" disabled={!!busy} onClick={() => void run(`wpause-${wave.id}`, () => api.pauseWave(wave.id).then(() => undefined))}>⏸</button>
                          )}
                          <button type="button" className="pm-btn sm" disabled={!!busy} onClick={() => void run(`wadv-${wave.id}`, () => api.advanceWave(wave.id).then(() => undefined))}>下一阶段</button>
                        </div>
                      )}
                    </div>
                  ))
                )}
              </div>
            </>
          )}
        </section>
      </div>
    </div>
  );
}
