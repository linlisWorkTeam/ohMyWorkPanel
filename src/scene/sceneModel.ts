/**
 * 像素场景的只读投影层。
 *
 * 场景不拥有任何业务状态：它把已有的 member / run 数据投影成「谁在哪里、在干什么」。
 * 保持纯函数，方便单测与后续替换成 Canvas 渲染。
 */

export type SceneAgentState = "idle" | "working" | "blocked" | "done";

export interface SceneMemberInput {
  id: string;
  name: string;
  role?: string;
  adapter?: string;
}

export interface SceneRunInput {
  memberId: string;
  /** run 状态字符串，来自现有调度投影（running / queued / failed / done ...）。 */
  status: string;
}

export interface SceneAgent {
  id: string;
  name: string;
  role: string;
  adapter: string;
  state: SceneAgentState;
  /** 工位序号，从 0 开始。 */
  seat: number;
  x: number;
  y: number;
}

export interface SceneLayout {
  originX: number;
  originY: number;
  columns: number;
  seatGapX: number;
  seatGapY: number;
  /** 空闲角色的候场区起始 y。 */
  loungeY: number;
  loungeGapX: number;
}

export const DEFAULT_SCENE_LAYOUT: SceneLayout = {
  originX: 64,
  originY: 96,
  columns: 4,
  seatGapX: 96,
  seatGapY: 88,
  loungeY: 384,
  loungeGapX: 56,
};

/**
 * run 状态 → 场景状态。未知状态按 idle 处理，避免新状态把角色画成"卡住"。
 */
export function sceneStateForRun(status?: string | null): SceneAgentState {
  switch (status) {
    case "running":
      return "working";
    case "queued":
      return "working";
    case "failed":
    case "error":
    case "canceled":
    case "cancelled":
      return "blocked";
    case "done":
    case "succeeded":
    case "completed":
      return "done";
    default:
      return "idle";
  }
}

/**
 * 把成员与 run 投影成场景角色。
 *
 * - 同一成员有多条 run 时，以最后一条为准（调用方按时间顺序传入）。
 * - 没有 run 的成员视为空闲，站在候场区。
 * - 布局是确定性的：同一输入永远得到同一坐标，便于测试与动画补间。
 */
export function projectSceneAgents(
  members: SceneMemberInput[],
  runs: SceneRunInput[],
  layout: SceneLayout = DEFAULT_SCENE_LAYOUT,
): SceneAgent[] {
  const latestStatus = new Map<string, string>();
  for (const run of runs) {
    latestStatus.set(run.memberId, run.status);
  }

  let loungeIndex = 0;
  return members.map((member, index) => {
    const status = latestStatus.get(member.id);
    const state = status === undefined ? "idle" : sceneStateForRun(status);
    const seat = index;

    if (state === "idle") {
      const x = layout.originX + loungeIndex * layout.loungeGapX;
      const y = layout.loungeY;
      loungeIndex += 1;
      return {
        id: member.id,
        name: member.name,
        role: member.role ?? "",
        adapter: member.adapter ?? "",
        state,
        seat,
        x,
        y,
      };
    }

    const column = seat % layout.columns;
    const row = Math.floor(seat / layout.columns);
    return {
      id: member.id,
      name: member.name,
      role: member.role ?? "",
      adapter: member.adapter ?? "",
      state,
      seat,
      x: layout.originX + column * layout.seatGapX,
      y: layout.originY + row * layout.seatGapY,
    };
  });
}

export interface SceneSummary {
  total: number;
  idle: number;
  working: number;
  blocked: number;
  done: number;
}

export function summarizeScene(agents: SceneAgent[]): SceneSummary {
  const summary: SceneSummary = {
    total: agents.length,
    idle: 0,
    working: 0,
    blocked: 0,
    done: 0,
  };
  for (const agent of agents) {
    summary[agent.state] += 1;
  }
  return summary;
}
