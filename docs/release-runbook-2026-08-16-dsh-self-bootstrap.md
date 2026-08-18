---
date: 2026-08-16
topic: release-runbook-dsh-self-bootstrap
status: checklist
---

# 发布 Runbook：DSH 自举接入（P0 + P2 切片）

> 目标流程：**本地调试 OK → 合入 GitHub → ECS 上的 WorkPanel 自举更新到灰度(:8081) → 灰度转生产(:8080)**。
> 本文件是给“有 shell / 网络”的执行环境用的操作清单；当前会话因 Windows bash 禁用 + 无外网，无法代为执行第 2 步之后的内容。

## 0. 范围（本批内容）

- **P0（已提交）**：`dsh` headless 适配器（`adapters/dsh.rs` + `AdapterKind::Dsh`）+ 群聊「跳转 DSH Web」内嵌 :3080。
- **P2 切片（本批新增）**：两级自举 Agent 的数据层与只读强制：
  - `Member.system_locked`（Rust 模型 + 前端 `systemLocked`）
  - 迁移 `agent_profiles.system_locked`（幂等 `ALTER TABLE`，启动自动生效）
  - WorkPanel 种子组 `linlis-super-harness`（`system_locked=1`，`ensure_default_seed` 自动落）
  - 后端守卫 `assert_member_mutable`（拒绝 remove/set_admin/改模型/改工作区）
  - 前端成员行只读 + 「系统」徽标
- **未做（后续立项）**：普通群极简 `bootstrap-dsh-<group>` 的逐群 seed、web.rs 同名守卫、UI-P0 三栏 AppFrame、P1 ACP 会话回放、P3 自举闭环。

## 1. 本地验证（必须全绿）

```bash
cd /AI/LinlisWorkPanel

# 前端类型与单测
pnpm install
pnpm test
pnpm run test:gate        # Vitest + Rust lib 门禁（含 Rust 单测）

# Rust（Tauri 后端）
cd src-tauri
cargo test --no-default-features --lib
cargo build                # 确认 Member 新增字段无遗漏字面量（member_from_row 已补）
cd ..

# Web 构建冒烟
pnpm run build:web
pnpm run build             # Vite 前端产物
```

**重点人工核对**
1. 启动后自动迁移：`agent_profiles.system_locked` 列存在（幂等 ALTER，无需手工 SQL）。
2. 种子组 `LinlisWorkPanel`（`is_system=1`）成员含 **linlis-super-harness**（adapter=dsh, system_locked=1），无“检测/设管理/移除/改模型/改工作区”入口，显示「系统」徽标。
3. 尝试对 linlis-super-harness 调 remove/set_admin/改模型 → 后端返回“平台锁定的自举 Agent 不可修改或移除”。
4. 普通成员不受影响（system_locked=0）。
5. `dsh web` 在 :3080 时，「跳转 DSH Web」可打开内嵌页面。

## 2. 合入 GitHub

```bash
cd /AI/LinlisWorkPanel
git add -A
# 按项目提交规范：类型前缀，中文/英文均可
git commit -m "feat: dsh 自举执行者 P0+P2 切片（适配器/嵌入/两级自举 Agent/system_locked）"
git push origin master
```

## 3. ECS：让 WorkPanel 自举更新灰度（:8081）

> ECS 上 WorkPanel 是通过“自举”机制拉新代码的。按 README 的 Web 运维路径：

```bash
# 在 ECS（工作区 /AI/LinlisWorkPanel）上
ssh <ecs-host>          # 进入 ECS
cd /AI/LinlisWorkPanel
git pull origin master   # 拉取本次发布内容（合入 GitHub 后）

export CARGO_BUILD_JOBS=1 NODE_OPTIONS=--max-old-space-size=1024
./scripts/deploy-canary.sh            # 门禁 → 构建 → 灰度 :8081 + data-canary
./scripts/canary-announce-a2a.sh      # 灰度群 A2A 公告改动点（按纪律）
```

**灰度验证（:8081）**
- 登录 :8081（默认 root/root），进入 WorkPanel 组：
  - 成员栏出现只读「系统」linlis-super-harness；
  - 建一个 dsh Agent 跑 `@` 任务（headless）；
  - 「跳转 DSH Web」可打开 :3080（ECS 上需先启动 `dsh web`）。
- 运行日志/经验/交接按现有检查项过一遍（参考 `docs/release-checklist.md` 前端壳冒烟 §F）。

## 4. 灰度转生产（:8080）

```bash
# 一次性批准（root 语义，15 分钟有效）
./scripts/approve-prod-release.sh "who: <执行人>; why: DSH 自举接入 P0+P2 切片(两级自举 Agent/system_locked)"

# 灰度 → 生产（不覆盖生产 DB；勿中断 stop→start）
./scripts/promote-canary.sh
```

> 纪律（勿漂移）：自举/自改类上线必须经上述群聊/审批语义；禁止 `systemctl restart` 生产时打断 stop→start；禁止伪造批准令牌；生产与灰度数据目录分离（data / data-canary 勿混用）。

## 5. 发布后核对（对外输出口径）

- 对外一句话：**“引入不可修改的两级 DSH 自举 Agent（普通群极简 bootstrap-dsh、WorkPanel 组 linlis-super-harness），把面板自我更新收敛到受人类审批、可审计、可回滚的单一通道。”**
- 需随版本输出的文档：`docs/superpowers/specs/2026-08-16-dsh-self-bootstrap-runtime.md`、`docs/superpowers/specs/2026-08-16-group-chat-governance-plane.md`、`docs/superpowers/specs/2026-08-16-dsh-ui-language-workpanel.md`。

## 6. 回滚预案

- 若 promote 后有问题：`dsh` 插件/扩展走运行时回滚（plugin unload）；面板本体回滚按现有双槽位（重新 promote 上一个 canary/生产 tar）→ 见 `docs/superpowers/specs/2026-08-15-smooth-release-drain-design.md`。
