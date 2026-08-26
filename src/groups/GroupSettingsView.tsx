import { useEffect, useState } from "react";
import { api } from "../api";
import { ServerPathPicker } from "./ServerPathPicker";
import type { Group, Member } from "../types";

interface Props {
  group: Group;
  members: Member[];
  canManage: boolean;
  onGroupPatch: (group: Group) => void;
  onMemberPatch: (member: Member) => void;
  onError: (msg: string) => void;
}

/** Group announcement + workspace settings (restored after V1.3.0 removed 项目 tab). */
export function GroupSettingsView({
  group,
  members,
  canManage,
  onGroupPatch,
  onMemberPatch,
  onError,
}: Props) {
  const [announcement, setAnnouncement] = useState(group.announcement ?? "");
  const [savingAnn, setSavingAnn] = useState(false);
  const [groupWs, setGroupWs] = useState(group.workspacePath);
  const [savingWs, setSavingWs] = useState(false);
  const [editingAgent, setEditingAgent] = useState<string | null>(null);
  const [agentWs, setAgentWs] = useState("");

  useEffect(() => {
    setAnnouncement(group.announcement ?? "");
  }, [group.id, group.announcement]);

  useEffect(() => {
    setGroupWs(group.workspacePath);
  }, [group.id, group.workspacePath]);

  const agents = members.filter((m) => m.kind === "agent" && m.isActive);

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

  const isChat = group.groupKind === "chat";

  return (
    <div className="project-workflow group-settings-view">
      <section className="wf-section">
        <h2>群公告（项目级规则）</h2>
        <p className="wf-hint">保存后注入所有 Agent 的 prompt，并同步到工作区 `.cursor/rules/group-announcement.mdc`。</p>
        <textarea
          className="wf-announce"
          rows={5}
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
        {!canManage && <p className="wf-hint">仅群主 / 平台管理员可修改。</p>}
      </section>

      {!isChat && (
        <section className="wf-section">
          <h2>群工作目录</h2>
          <p className="wf-hint">Agent 默认在此服务器绝对路径下执行；须为已存在目录。</p>
          <div className="wf-workspace">
            <p className="wf-meta">当前：<code>{group.workspacePath || "（未设置）"}</code></p>
            {canManage && (
              <div className="wf-ws-edit">
                <ServerPathPicker value={groupWs} onChange={setGroupWs} onError={onError} />
                <button
                  className="pm-btn sm"
                  disabled={savingWs || groupWs === group.workspacePath}
                  onClick={() => void saveGroupWorkspace()}
                >
                  {savingWs ? "保存中…" : "更新群工作区"}
                </button>
              </div>
            )}
          </div>
        </section>
      )}

      {!isChat && (
        <section className="wf-section">
          <h2>Agent 工作区</h2>
          <p className="wf-hint">可覆盖单个 Agent 的沙箱路径（须落在群工作区下）。</p>
          <div className="agent-lanes">
            {agents.map((agent) => (
              <div key={agent.id} className="agent-lane">
                <div className="lane-head">
                  <span className="lane-dot" style={{ background: agent.avatarColor }} />
                  <strong>{agent.displayName}</strong>
                  <small>{agent.adapter ?? "—"}</small>
                </div>
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
                      <button
                        className="pm-btn sm quiet"
                        onClick={() => {
                          setEditingAgent(agent.id);
                          setAgentWs(agent.workspacePath || group.workspacePath);
                        }}
                      >
                        改工作区
                      </button>
                    )
                  )}
                </div>
              </div>
            ))}
            {agents.length === 0 && <p className="wf-hint">暂无 Agent</p>}
          </div>
        </section>
      )}
    </div>
  );
}
