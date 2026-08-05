---
date: 2026-08-05
topic: frontend-outage-lessons
branch: master
status: active
---

# Epitaph: 前台「React 崩掉」类故障经验

## 背景

近期多次出现「前台坏了 / 像 React 崩了」的反馈。复盘后，多数不是单一 React 组件逻辑，而是 **运行时壳层 + 发布操作** 问题；真正的 render 抛错也会因缺少 ErrorBoundary 表现为整页白屏。

## 已确认案例

1. **生产停服（2026-08-05）**  
   promote 过程中 `systemctl restart` 被中断 → stop 成功、start 未完成 → 服务 dead，浏览器侧等同前台全挂。  
   处理：`promote-canary.sh` 改为显式 stop/start 并检查 active；proxy 对 prod 用 `Wants=` 避免连锁停死。

2. **HTTPS 下实时 UI「假死」（2026-08-03）**  
   前端写死 `ws://`，HTTPS 站点混合内容拦截 → Agent/Chatbot 有结果但不推送到前台，需刷新/切群才看到。  
   处理：按页面协议选 `wss`/`ws`，终态 `run_status` 补 refresh（`7d932c8`）。

3. **发版后静态资源错配风险**  
   Vite hashed assets + PWA SW：若 HTML/JS 版本不一致，表现为白屏或 dynamic import 失败，常被叫成「React 崩了」。  
   缓解：SW 对 `/assets/` network-first；发版用 `docs/release-checklist.md` §F 校验 JS/CSS 200。

## Do not regress

- promote 结束后生产 **必须 active**，并做 §F 前端壳冒烟
- 改实时链路必须在 **HTTPS 域名**测，不能只测本机 HTTP
- 发布检查清单：`docs/release-checklist.md`（后续发版必跟）

## Follow-ups（非本次必做）

- 前端加 React ErrorBoundary，避免单气泡异常打挂整树
- 可选自动化：login + groups + WS + asset 存在性脚本挂到 promote 末尾
