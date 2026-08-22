---
date: 2026-08-06
topic: extension-host-v130
status: active
---

# Epitaph：通用扩展宿主（V1.3.0 EH）

## 做了什么

- 发现：`OHMYWORKPANEL_EXTENSION_ROOTS` + panellive 回落；多扩展 `GET/PUT .../extensions[/{id}]`
- 反代：`ANY /api/extensions/{extId}/{*path}` → `proxy_extension`
- 前端：`collectExtensionTabViews` + `ExtensionPanel`；设置 Extend 多开关
- A2A：按 manifest `a2aSkills` 校验；非 live skill 宿主 ack
- 门禁：`scripts/check-extension-purity.sh` 进 `test:gate`
- 文档：`docs/extensions/EXTENSION_GUIDE.md`、claim、version-pipeline、V1.3.0 design

## 视觉 E2E（2026-08-07 canary :8081）

- 截图：`assets/e2e-visual/`（登录→群→Live→酒馆→设置 Extend）
- 顶栏：`聊天 | Live | 酒馆`；状态「Extend 2/2 就绪」
- 酒馆 iframe：`/api/extensions/ai-hotel/tavern.html?groupId=…`（同源反代，非直连端口）
- 修复：`App.tsx` 将 Ask/version `useEffect` 移到 auth 早退之前，消除登录后 React #310 白屏

## 未完成 / 风险

- 酒馆页视觉桩为临时 mock（`:8791`）；@OpenClaw 真引擎替换后需再验
- PanelLive 语音按住说话未做自动点击冒烟（页签/iframe 已加载）
- `web.rs` 仍保留 panellive events / session 钩子（迁移期）
- 改动仍在工作区；提交时勿混入无关 `scheduler.rs`

## 下一步

- root 批准后 commit（排除 scheduler）→ 再批 promote 生产
