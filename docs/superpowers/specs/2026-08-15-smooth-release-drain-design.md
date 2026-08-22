# Design: 平滑发版 Drain + 重启恢复（方案 A）

**日期**: 2026-08-15  
**状态**: 实现中

## 行为

1. **Drain 模式**（`app_settings.release_drain=1`）  
   - 拒新建 task_run（@Agent 消息仍入库，`runIds=[]` + `drainActive`）  
   - 调度不启动 queued；已 running 继续跑完  
2. **发版脚本**在 `systemctl stop/restart` 前：`drain-wait`（登录 → enable drain → 等到 running=0 或超时）  
3. **进程启动 `init_db`**：`queued/running` → 重新 **`queued`**（`phase=recovering`），streaming 消息标 interrupted；清除 drain  
4. 后台 scheduler 3s 后扫 queued → 自动开跑（Cursor 仍可用 `cli_session_id` resume）

## 非目标（本切片）

- 不保证同一 CLI 子进程续跑  
- 不恢复半截 streaming 气泡内容（新 run 新气泡）

## 风险

- 超时强停仍会砍 running，靠重启重入队（可能重复工具副作用）  
- Cursor session 失效时走既有 clear+retry
