import { describe, expect, it } from "vitest";
import {
  DEFAULT_SCENE_LAYOUT,
  projectSceneAgents,
  sceneStateForRun,
  summarizeScene,
  type SceneMemberInput,
} from "./sceneModel";

const members: SceneMemberInput[] = [
  { id: "m1", name: "Codex", role: "admin", adapter: "codex" },
  { id: "m2", name: "Claude", role: "member", adapter: "claude" },
  { id: "m3", name: "OpenCode", role: "member", adapter: "opencode" },
];

describe("sceneStateForRun", () => {
  it("maps调度状态到场景状态", () => {
    expect(sceneStateForRun("running")).toBe("working");
    expect(sceneStateForRun("queued")).toBe("working");
    expect(sceneStateForRun("failed")).toBe("blocked");
    expect(sceneStateForRun("canceled")).toBe("blocked");
    expect(sceneStateForRun("done")).toBe("done");
    expect(sceneStateForRun(undefined)).toBe("idle");
    expect(sceneStateForRun("some-new-status")).toBe("idle");
  });
});

describe("projectSceneAgents", () => {
  it("没有 run 的成员站在候场区", () => {
    const agents = projectSceneAgents(members, []);
    expect(agents).toHaveLength(3);
    for (const agent of agents) {
      expect(agent.state).toBe("idle");
      expect(agent.y).toBe(DEFAULT_SCENE_LAYOUT.loungeY);
    }
  });

  it("有活跃 run 的成员坐在工位并标记为 working", () => {
    const agents = projectSceneAgents(members, [{ memberId: "m2", status: "running" }]);
    const claude = agents.find((agent) => agent.id === "m2");
    expect(claude?.state).toBe("working");
    expect(claude?.y).toBe(DEFAULT_SCENE_LAYOUT.originY);
  });

  it("同一成员多条 run 以最后一条为准", () => {
    const agents = projectSceneAgents(members, [
      { memberId: "m1", status: "running" },
      { memberId: "m1", status: "done" },
    ]);
    expect(agents.find((agent) => agent.id === "m1")?.state).toBe("done");
  });

  it("布局是确定性的", () => {
    const first = projectSceneAgents(members, [{ memberId: "m1", status: "running" }]);
    const second = projectSceneAgents(members, [{ memberId: "m1", status: "running" }]);
    expect(first).toEqual(second);
  });

  it("工位按列换行", () => {
    const crowd: SceneMemberInput[] = Array.from({ length: 6 }, (_, index) => ({
      id: `c${index}`,
      name: `成员${index}`,
    }));
    const agents = projectSceneAgents(
      crowd,
      crowd.map((member) => ({ memberId: member.id, status: "running" })),
    );
    const columns = DEFAULT_SCENE_LAYOUT.columns;
    expect(agents[0].x).toBe(DEFAULT_SCENE_LAYOUT.originX);
    expect(agents[1].x).toBe(DEFAULT_SCENE_LAYOUT.originX + DEFAULT_SCENE_LAYOUT.seatGapX);
    expect(agents[columns].x).toBe(DEFAULT_SCENE_LAYOUT.originX);
    expect(agents[columns].y).toBe(DEFAULT_SCENE_LAYOUT.originY + DEFAULT_SCENE_LAYOUT.seatGapY);
  });
});

describe("大场景", () => {
  it("20 个成员的位置互不重叠", () => {
    const crowd = Array.from({ length: 20 }, (_, index) => ({
      id: `c${index}`,
      name: `成员${index}`,
    }));
    const agents = projectSceneAgents(
      crowd,
      crowd.map((member) => ({ memberId: member.id, status: "running" })),
    );
    const positions = new Set(agents.map((agent) => `${agent.x},${agent.y}`));
    expect(positions.size).toBe(20);
  });

  it("20 个空闲成员在候场区换行且不越界", () => {
    const crowd = Array.from({ length: 20 }, (_, index) => ({
      id: `c${index}`,
      name: `成员${index}`,
    }));
    const agents = projectSceneAgents(crowd, []);
    const positions = new Set(agents.map((agent) => `${agent.x},${agent.y}`));
    expect(positions.size).toBe(20);
    for (const agent of agents) {
      expect(agent.x).toBeLessThanOrEqual(720);
      expect(agent.y).toBeLessThanOrEqual(520);
    }
  });
});

describe("summarizeScene", () => {  it("统计各状态数量", () => {
    const agents = projectSceneAgents(members, [
      { memberId: "m1", status: "running" },
      { memberId: "m2", status: "failed" },
      { memberId: "m3", status: "done" },
    ]);
    expect(summarizeScene(agents)).toEqual({
      total: 3,
      idle: 0,
      working: 1,
      blocked: 1,
      done: 1,
    });
  });
});
