# 发版检查清单（灰度 → 生产）

与群公告一致：**更新 docs → 灰度验证 → commit → promote 生产**。  
本清单吸收近期「前台像崩掉」类故障，发版时必须勾选。

## 0. 变更与文档

- [ ] 行为变更已有对应测试或明确「无行为变更 / N/A」
- [ ] 相关 docs 已更新（至少 `docs/api-web.md` / epitaph / 本清单若触及发布流程）
- [ ] `pnpm run test:gate` exit 0

## 1. 灰度部署（`:8081`）

```bash
./scripts/deploy-canary.sh
systemctl is-active linlis-work-panel-canary.service
# 部署灰度后生产仍须 active
systemctl is-active linlis-work-panel.service
```

- [ ] canary HTTP `/` → 200
- [ ] **前端壳冒烟（防白屏 / React 起不来）** — 见 §F
- [ ] root 登录；关键群可见
- [ ] **A2A 灰度改动公告（必做）** — 每次 `deploy-canary` 成功后，在灰度环境「灰度测试」群经 A2A `@` 该群管理员，推送本次改动点：
  ```bash
  ./scripts/canary-announce-a2a.sh
  # 或自定义摘要：
  ./scripts/canary-announce-a2a.sh $'-\n- 修复登录白屏 React #310\n- …'
  ```
  约定：目标群名默认 `灰度测试`（`:8081` / `data-canary`）；由群主发消息 `@管理员 Agent`，管理员在群内复述/确认改动清单；调用嵌套 ≤3 层。
- [ ] 若改了聊天/实时：HTTPS 入口测一轮 @Agent，确认流式更新（勿只测本机 `http://127.0.0.1`）
- [ ] 生产未被误停（`prod` service active，`/` → 200）

## 2. 生产晋升（`:8080`）

```bash
./scripts/approve-prod-release.sh "简述原因"
./scripts/promote-canary.sh   # 内部 stop→start；勿用易中断的裸 restart 半截
```

- [ ] promote 结束后 `linlis-work-panel.service` **active**（不是 dead）
- [ ] `/` → 200；root 登录；**LinlisWorkPanel** 群仍在
- [ ] §F 前端壳冒烟在 **生产** 再跑一遍
- [ ] 若经域名/HTTPS：再确认实时流（`wss`）一次
- [ ] auth proxy / nginx（若启用）仍可访问对外入口
- [ ] **未**覆盖 `/AI/LinlisWorkPanel/data`
- [ ] （可选）生产 watchdog 已启用：`linlis-work-panel-watchdog.timer`（`scripts/ensure-prod-up.sh`，仅拉起死服务，不 promote）

`promote-canary.sh` 在 stop 之后设有 EXIT/INT/TERM trap，中断时仍会尝试 `systemctl start` 生产，避免永久 dead。

## F. 前端壳冒烟（React / 静态资源）

「前台崩了」常见并不是 React 逻辑 bug，而是 **壳起不来 / 资源错配 / 服务未拉起**。每次发版至少做：

```bash
PORT=8081   # 或 8080
# 1) HTML 可达
curl -sS -o /dev/null -w '%{http_code}\n' "http://127.0.0.1:${PORT}/"
# 2) index 引用的 hashed JS/CSS 真实存在（避免旧 HTML + 新 assets 或 SW 错配导致白屏）
JS=$(curl -sS "http://127.0.0.1:${PORT}/" | sed -n 's/.*src="\(\/assets\/[^"]*\.js\)".*/\1/p' | head -1)
CSS=$(curl -sS "http://127.0.0.1:${PORT}/" | sed -n 's/.*href="\(\/assets\/[^"]*\.css\)".*/\1/p' | head -1)
curl -sS -o /dev/null -w "js=%{http_code}\n" "http://127.0.0.1:${PORT}${JS}"
curl -sS -o /dev/null -w "css=%{http_code}\n" "http://127.0.0.1:${PORT}${CSS}"
```

人工浏览器（灰度优先）：

- [ ] 硬刷新后能进登录/主界面（非空白 `#root`）
- [ ] DevTools Console **无** `Minified React error` / `Failed to fetch dynamically imported module` / 未捕获异常导致整页挂掉
- [ ] 经 **HTTPS 域名**访问时，Network 里 WebSocket 为 **`wss://`**（不能是被拦截的 `ws://`）
- [ ] 发一条短消息或 @Agent，列表/气泡有更新（不只刷新后才出现）

## 经验摘要（勿再踩）

| 现象 | 根因 | 检查 |
|---|---|---|
| 整站打不开 / 像前台崩了 | promote 时 `systemctl restart`/`stop` 后未及时 `start` 被中断，生产停在 dead | promote **全程勿中断**；用 stop→start；promote 后必须 `is-active`；看 journal 是否出现长间隔 Stopped→Started |
| promote 后「挂了」几分钟 | Agent/SSH 会话在 `stop` 与 `start` 之间断开（本机仅 ~2G 内存时更易拖死） | 先拷贝产物再切服务；窗口内禁止并行重活；机器内存打满时先减负再 promote |
| 页面在但 Agent 输出不实时 | HTTPS 页用了 `ws://`，被混合内容拦截 | 域名入口测 `wss`；见 commit `7d932c8` |
| 发版后白屏 / chunk 404 | HTML 与 `/assets/*` 哈希不一致或 SW 旧缓存 | §F 校验 JS/CSS 200；SW 对 assets 为 network-first |
| 单组件异常整页挂 | 无 ErrorBoundary 兜底 | Console 查堆栈；修 render 空指针后再发 |
| 登录后整页白屏（`#root` 空）+ Console `React #310` | 鉴权 early return（checking/login）**之后**又挂了 `useEffect`/`useState`，login→ready 钩子数量变化 | 所有 hooks 必须放在 auth early return **之前**（见 `App.tsx` 注释） |

详见 epitaph：`docs/epitaph/2026-08-05-frontend-outage-lessons.md`。
