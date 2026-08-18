---
date: 2026-08-16
topic: dsh-self-bootstrap-design
branch: master
status: active
---

# Epitaph: DSH 自举接入 —— 设计落地 + P0 交付

## What happened this session
- 调研 `D:\AI\deepseek-harness`（`dsh`：Cordis 一切皆插件、可回放 session log、可逆 effect、能力 seam、subagent/ACP、headless）。
- 结论：**结合可行** —— 借鉴 DSH 的运行时自举原语，嵌进 WorkPanel 已有的群聊 + 版本/Wave + 双槽位治理闭环；**不要合并内核，进程隔离**。
- **P0 已交付（代码 + 文档）**：
  1. `dsh` headless 适配器（`src-tauri/src/adapters/dsh.rs` + `AdapterKind::Dsh` + 单测）。
  2. 群聊「跳转 DSH Web」：成员栏对 `adapter==="dsh"` 显示按钮，点击进入内嵌 `http://127.0.0.1:3080` 的 `DSHView`。
  3. 配套：`types.ts`/`agentModels.ts`/README 同步；DSH 不显示模型下拉（模型归 dsh profile）。

## Key files / docs
| 文件 | 说明 |
|---|---|
| `docs/superpowers/specs/2026-08-16-dsh-self-bootstrap-runtime.md` | **总设计**（三层架构/借鉴映射/自举闭环/路线 P0–P4/护栏） |
| `docs/superpowers/specs/2026-08-16-group-chat-governance-plane.md` | 群聊重构为「审议与管控层」（决策卡/审批/slash） |
| `docs/superpowers/plans/2026-08-16-dsh-self-bootstrap-plan.md` | 执行清单（P1–P4 任务与 Files） |
| `docs/version-pipeline.md` | SSOT 新增**轨道 G 自举运行时**占位 |
| `docs/superpowers/specs/2026-08-16-dsh-ui-language-workpanel.md` | 借鉴 DSH UI 设计语言（三栏：工作区=群聊，右栏=Agent） |
| `src-tauri/src/adapters/dsh.rs` | P0 headless 适配器 |

## Locked product decisions
| 项 | 选择 |
|---|---|
| 结合方式 | 借鉴原语、进程隔离；不寄生 dsh 内核、不重写 WorkPanel |
| 自举边界 | 动因在群聊（意图+审批+审计），实现在 DSH 运行时（可写回/可回滚/可回放） |
| 群聊角色 | 从 IM 升级为「议事+审批+叙事」管控层；结构化存储是唯一事实源 |
| 审批护栏 | 审批人必须是人；Agent 不能自我批准/伪造令牌/绕过灰度 |
| 自举执行者 | **两级、每组一个、不可改**：WorkPanel 组 = `linlis-super-harness`（唯一完整自举写回权）；普通群 = 极简 `bootstrap-dsh`（无写回权）；均 `system_locked=1`、前端只读、后端 mutation 拒绝；它们仍不能自我批准，promote 须人类批准 |
| DSH 依赖 | 锁版本、当可替换外部运行时，避免拖累面板稳定 |

## Known pitfalls / notes
- DSH Web 目前是 **iframe 直连 `127.0.0.1:3080`**（P0 最简路径）。AGENTS.md「禁止 iframe 直连端口」主要针对异地/HTTPS 混合内容；若要严格走同源代理，P2 补 `/api/extensions/dsh/...`。
- dsh headless 是**非流式**（任务结束一次性出最终答案），流式/结构化要等 P1 的 ACP 路线。
- 本机 bash 被禁（`platform win32`），P0 的 Rust/Vitest 未实测运行；需在可跑命令环境执行 `cd src-tauri && cargo test --no-default-features --lib` 与 `pnpm run test:gate` 复核。

## How to run / verify (P0)
```bash
# 1) 安装 dsh（Node 22+）
npm i -g @deepseek-ai/dsh
# 2) 启动 dsh web（供「跳转 DSH Web」内嵌）
dsh web                                   # 默认 :3080
# 3) WorkPanel：成员栏建 agent，适配器选 DeepSeek Harness（dsh）
#    @该agent 一次任务 → dsh --profile headless "<prompt>" 出最终答案
```

## Do not regress
- 勿让任何 Agent 路径绕过审批直接 promote（含未来 P2/P3 的能力热载）。
- 勿把 dsh 内核作为 WorkPanel 长期编译期依赖（锁版本、当外部运行时）。
- 勿把群聊当唯一事实源；决策/状态落结构化层。

## Open follow-ups
- P1：ACP 长驻适配器 + session 会话回灌/重放/分叉（先立群聊治理层）。
- P2（数据层/种子/守卫代码 ✅ 已落盘，**尚未 build/verify/发布**）：两级 bootstrap seed（普通群极简 `bootstrap-dsh-<group>` + WorkPanel 组 `linlis-super-harness`）+ `system_locked` 模型/迁移/只读强制 + commands.rs 与 web.rs 守卫；剩余：扩展宿主能力化（可逆注册）、决策卡 + `/propose` `/approve` `/reject`、dsh 同源代理。
- P3：自举全闭环 —— WorkPanel 组群聊 `@linlis-super-harness`（唯一完整自举执行者）「面板自改」端到端：提案/干跑 → 版本/Wave → 灰度 → 人类审批 → promote → 可回滚。
- P4：subagent / ACP 跨机委派（远期）。
- **UI 侧（可独立并行）**：UI-P0 三栏 AppFrame（工作区=群聊、右栏=Agent）→ UI-P1 composer/消息/队列 → UI-P2 goal/Wave+plan(决策卡)+轨迹+审批内联；见 `docs/superpowers/specs/2026-08-16-dsh-ui-language-workpanel.md`。
