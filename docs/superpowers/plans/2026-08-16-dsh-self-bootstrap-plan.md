# Plan: DSH 自举接入执行计划

> Spec: `docs/superpowers/specs/2026-08-16-dsh-self-bootstrap-runtime.md`
> 治理层 Spec: `docs/superpowers/specs/2026-08-16-group-chat-governance-plane.md`

## 状态

- **P0 ✅ 已完成**：`dsh` headless 适配器（`adapters/dsh.rs`）+ 群聊「跳转 DSH Web」内嵌 :3080。
- **P2 ⚠️ 代码已齐，待 build/verify**：`system_locked`（模型/迁移/`MEMBER_SELECT`/`member_from_row`）；ohMyWorkPanel 组 `linlis-super-harness` seed；普通群极简 `bootstrap-dsh-<group>` seed；commands.rs + web.rs 五处成员 mutation 守卫；前端成员行只读+「系统」徽标。**尚未编译/测试，也未发布**。
- **P1 / P3 / UI-P0**：待立项（见计划任务与 UI spec）。
- **不变式**：先立「群聊治理层」再上能力热载；DSH 保持进程隔离 + 锁版本。

> 发布执行（build/test → git push → ECS 灰度 :8081 → 生产 :8080）见 `docs/release-runbook-2026-08-16-dsh-self-bootstrap.md`；当前会话因 Windows bash 禁用 + 无外网无法代执行。

## Files（涉及面预估）

- 后端：`adapters/`（新 `acp.rs`、`models.rs`）、`scheduler.rs`、`db.rs`（新 `decision`/`session_replay` 表）、`commands.rs`、`web.rs`（ACP 代理、决策卡 API、/api/extensions/dsh 同源代理）、`extensions.rs`（能力注册/可逆 effect）、`main_server.rs`、`lib.rs`
- 前端：`types.ts`、`api.ts`、`api-web.ts`、`App.tsx`（决策卡 UI、slash 命令、会话重放入口）、`GroupSettingsView.tsx`、`VersionView.tsx`、`styles.css`、tests

## Tasks（按阶段）

### P1 —— ACP 会话回灌与重放
1. 新增 ACP 长驻适配器（`adapters/acp.rs`）：spawn dsh ACP 进程，走 `initialize/session/new/session/prompt/session/update/cancel`。
2. `session/update` 的 `agent_message_chunk` 回灌 → ohMyWorkPanel `task_run`/`message`/`run_event`；保留 `session_ref` 句柄。
3. 群聊内该「会话重放」视图 + 一键分叉（把分叉后的新 run 挂到原版本下）。
4. 全量留痕限窗口（参考滚动摘要），避免存储膨胀。
5. 门禁：`cargo test --lib` + Vitest + 灰度 smoke。

### P2 —— 预制两级 bootstrap（不可改）+ 扩展宿主能力化 + 可逆注册
1. **预制两级模板**：
   - 普通群/新建项目群 seed 自动创建极简 `bootstrap-dsh-<group>`（`adapter=dsh`，**无自举写回权**）。
   - ohMyWorkPanel 组（`is_system=1`）创建 `linlis-super-harness`（`adapter=dsh`，**唯一完整自举写回权**）。
   - 两者 `agent_profiles.system_locked=1`。
2. **不可改强制**：前端成员行对 `system_locked` 只读；后端 `remove_member`/`set_admin`/`update_member_model_cmd`/`update_member_workspace_cmd` 一律拒绝；健康检查 + 就绪态展示。
3. `extensions.rs` 引入「能力注册」抽象：注册/反注册成对（diff → dry-run → apply → rollback）。
4. DSH 插件/manifest 写回：允许灰度槽热载「工具/适配器/服务」类扩展（UI 页签沿用现有同源代理）；写回能力**只挂 ohMyWorkPanel 组 `linlis-super-harness`**，普通群极简 `bootstrap-dsh` 无写回权。
5. 决策卡落地：`decision` 表 + `/propose` `/approve` `/reject` slash + 版本页来源展示。
6. 治理护栏：审批人必须是人；无任何 Agent 路径能绕过 `manually_approved` 直接 promote。
7. 同源代理 `/api/extensions/dsh/...`（补上 P0 未做的直连治理）。

### P3 —— 自举闭环（统一经 linlis-super-harness 执行）
1. ohMyWorkPanel 组「面板自改」：建议 → `@linlis-super-harness`（唯一完整自举执行者）提案/干跑 → 版本/Wave 立项 → 灰度（热载 DSH 写回能力）→ 审批 → promote → 可回滚。
2. 把 `approve-prod-release.sh` 群化为群聊内的审批动作（`/approve`），保留 root 一次性批准语义。
3. 端到端演示：通过 ohMyWorkPanel 组群聊 `@linlis-super-harness` 给面板加一个自带扩展并上线。

### P4 —— subagent / 跨机委派（远期）
1. 子任务可委托到另一进程/另一机器/另一产品的 ACP 端点。
2. 引入凭据/信任模型（跨机 token 与白名单）。

## 里程碑验收

- P1 后：一次 `@dsh` 任务结束可展开会话重放并可分叉。
- P2 后：面板能热载一个 dsh 写回的工具扩展；决策卡 + 审批全留痕。
- P3 后：全程「群聊决策卡 + 灰度 + 审批 + 可回滚」走通一次真实自更新。
