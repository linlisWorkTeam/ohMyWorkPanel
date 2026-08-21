# Web API 索引（薄参考）

契约以 `src-tauri/src/web.rs` 为准；本文只做路由查阅，不维护完整 schema。

## 鉴权

| 路径 | 鉴权 |
|---|---|
| `POST /api/auth/register`、`POST /api/auth/login` | 公开 |
| 其余 `/api/*`、`/ws` | `Authorization: Bearer <JWT>`（login 返回） |

字段命名：JSON 多为 **camelCase**。

## 文件系统（服务器本机）

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/fs/list?path=` | 列目录；空或 `/` 从根浏览 |
| POST | `/api/fs/mkdir` | body `{ parent, name }` → `{ path }`；`name` 单段、禁穿越；不可在 `/` 下创建 |

选用路径须为服务器绝对路径（非浏览器本机）。可先浏览已有目录，再在其下新建文件夹。

## 群组 / 成员

| 方法 | 路径 |
|---|---|
| GET/POST | `/api/groups`（GET 含 `unreadCount`；未读群优先排序） |
| PUT | `/api/groups/{id}/read`（进群清未读） |
| GET | `/api/presence`（`onlineUserIds`） |
| GET | `/api/groups/{id}` |
| GET/PUT | `/api/groups/{id}/announcement` |
| PUT | `/api/groups/{id}/workspace` |
| PUT | `/api/groups/{id}/archive` |
| GET/POST | `/api/groups/{group_id}/members`（GET 只读成员列表且不改变未读；POST `invite:true` → 待接受用户 + `inviteUrl`） |
| DELETE | `/api/groups/{group_id}/members/{member_id}`（软移除 `is_active=0`） |
| DELETE | `/api/groups/{group_id}/members/{member_id}/purge`（永久删除/隐藏 roster） |
| GET | `/api/invites/{token}`（无鉴权；邀请预览） |
| POST | `/api/invites/{token}/accept`（JWT；绑定当前用户） |
| GET | `/api/users/joinable?groupId=`（管理员；尚未加入该群的登录用户） |
| GET | `/api/groups/{id}/extensions`（Extend 列表 + health；`baseUrl` 为同源代理前缀） |
| PUT | `/api/groups/{id}/extensions/panellive` body `{ enabled }`（未就绪 → 409） |
| GET/POST | `/api/extensions/panellive/{*path}`（同源反代 PanelLive `:8790`，无 JWT） |
| POST | `/api/extensions/panellive/events`（`X-Panellive-Token`；禁 PCM；仅 WS，不写群消息） |
| POST | `/api/a2a/dispatch`（Live skills；禁 PCM；stop→cancel） |
| PUT | `/api/members/{member_id}/model` |
| PUT | `/api/members/{member_id}/workspace` |
| PUT | `/api/groups/{group_id}/admin`（`memberId`：活跃 agent **或 chatbot**；`null` 清除。聊天群=默认响应者；未设则无 @ 不兜底） |

添加 `kind=user` 成员时：

- **创建新账号**：`loginUsername` + `loginPassword`（用户名冲突返回占用错误）
- **加入已有账号**：`existingAuthUserId`（`users.id`）；无需密码；已在本群则 409
- **邀请链接**：`invite: true`（无需登录字段）；返回 `inviteUrl`/`inviteToken`/`inviteExpiresAt`（24h）；成员 `invitePending` 直至接受

## 消息 / 任务

| 方法 | 路径 |
|---|---|
| POST | `/api/messages` |
| GET | `/api/groups/{group_id}/messages` |
| GET | `/api/groups/{group_id}/messages/{message_id}/parts/{channel}` |
| GET | `/api/groups/{group_id}/runs` |
| GET | `/api/groups/{group_id}/runs/active`（queued/running，重连 resync） |
| POST | `/api/runs/{run_id}/cancel` |
| POST | `/api/runs/{run_id}/retry` |
| GET | `/api/health`（无鉴权；发布/重连探活） |
| GET | `/api/metrics/latest`（主进程 RSS/CPU；设置页 5s 拉） |

## 设置 / OCR / 预设角色

| 方法 | 路径 |
|---|---|
| GET/PUT | `/api/settings`（含心跳 Auto / 聚焦秒 / 后台秒） |
| POST | `/api/ocr`、`/api/ocr/base64` |
| GET | `/api/preset-roles` |
| GET | `/api/adapters`（CLI 目录：内置 ∪ `LINLIS_ADAPTER_ROOTS` 的 `*.adapter.json`） |

## 路线图 / 特性 / 编排

| 方法 | 路径 |
|---|---|
| GET | `/api/groups/{group_id}/roadmap` |
| POST | `/api/roadmap-items` |
| PUT/DELETE | `/api/roadmap-items/{id}` |
| POST | `/api/roadmap-items/{id}/start` |
| GET | `/api/groups/{group_id}/roadmap-orchestrations` |
| POST | `/api/roadmap-orchestrations/{id}/pause\|resume\|cancel` |
| GET | `/api/groups/{group_id}/features` |
| POST | `/api/features` |
| PUT/DELETE | `/api/features/{id}` |
| GET | `/api/features/{feature_id}/tasks` |
| POST | `/api/feature-tasks` |
| PUT/DELETE | `/api/feature-tasks/{id}` |
| GET | `/api/groups/{group_id}/roadmap-state` |

## 日志 / 经验 / Ops

| 方法 | 路径 |
|---|---|
| GET/DELETE | `/api/logs` |
| GET | `/api/logs/count` |
| POST/GET | `/api/experiences` |
| DELETE | `/api/experiences/{id}` |
| GET | `/api/ops/release-status`、`/api/ops/job` |
| POST | `/api/ops/test-gate`、`/api/ops/deploy-canary`（promote 仍用脚本，不进 UI） |

## 实时

| 路径 | 说明 |
|---|---|
| `GET /ws` | WebSocket；需鉴权 |

## 相关

- 前端封装：`src/api-web.ts`
- 目录逻辑：`src-tauri/src/fs_browse.rs`
- 发布流程：群公告 + `docs/epitaph/2026-08-01-v1.3-prod-canary.md` + `docs/release-checklist.md`
- 灰度推包后 A2A 公告：`scripts/canary-announce-a2a.sh`（在 canary「灰度测试」群 `@` 管理员推送改动点）
