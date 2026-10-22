import { useMemo, useState } from "react";
import type { Member, TaskRun } from "../types";
import { PixelScene } from "./PixelScene";
import {
  projectSceneAgents,
  summarizeScene,
  type SceneMemberInput,
  type SceneRunInput,
} from "./sceneModel";

export interface SceneViewProps {
  members: Member[];
  runs: TaskRun[];
  onExit: () => void;
  onSelectAgent?: (agentId: string) => void;
  /** 房间种子（由群 id 派生）：决定地板色带分区。 */
  roomSeed?: number;
}

/**
 * 像素场景视图：只读消费既有成员与 run 数据，不新增后端接口。
 * 进入方式由主壳的 `mainView` 决定；退出即回到聊天，不改变任何业务状态。
 */
export function SceneView({ members, runs, onExit, onSelectAgent, roomSeed }: SceneViewProps) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const agents = useMemo(() => {
    const memberInputs: SceneMemberInput[] = members.map((member) => ({
      id: member.id,
      name: member.displayName,
      role: member.roleDescription,
      adapter: member.adapter ?? "",
    }));
    const runInputs: SceneRunInput[] = [...runs]
      .sort((left, right) => left.createdAt - right.createdAt)
      .map((run) => ({ memberId: run.agentMemberId, status: run.status }));
    return projectSceneAgents(memberInputs, runInputs);
  }, [members, runs]);

  const summary = summarizeScene(agents);

  return (
    <div className="scene-view">
      <div className="scene-head">
        <h2>Agent 游乐园</h2>
        <div className="scene-hud">
          <span>在场 {summary.total}</span>
          <span data-state="working">工作 {summary.working}</span>
          <span data-state="blocked">阻塞 {summary.blocked}</span>
          <span data-state="done">完成 {summary.done}</span>
        </div>
        <button type="button" className="scene-exit" onClick={onExit} title="返回聊天">
          返回聊天
        </button>
      </div>
      {agents.length === 0 ? (
        <div className="scene-empty">还没有成员。添加 Agent 后，它们会出现在这里。</div>
      ) : (
        <PixelScene
          agents={agents}
          selectedId={selectedId}
          roomSeed={roomSeed}
          onSelectAgent={(agentId) => {
            setSelectedId(agentId);
            onSelectAgent?.(agentId);
          }}
        />
      )}
    </div>
  );
}
