---
date: 2026-08-21
topic: cli-adapter-manifest
epitaph: true
status: active
---

# 交接：内部多 CLI → 适配器 Manifest（设计已锁定）

## 一句话
SSOT：[`docs/superpowers/specs/2026-08-21-cli-adapter-manifest.md`](../superpowers/specs/2026-08-21-cli-adapter-manifest.md)（**accepted**）。做在 WorkPanel；终态 CLI 全声明化；mock/chatbot 不进 manifest。代码未开工。

## Locked（与规格 L1–L9 一致）
- 宿主 WorkPanel，`run_streaming` 唯一 CLI 入口；禁止 `sh -c`。
- 异步切片到删枚举；P0 现网零回归。
- mock 保留；chatbot 与 CLI 调度平级、执行分叉。
- 不进 connector；不编 dsh 内核；配置包无登录态。

## 下一步
P0 查表 spawn + `GET /api/adapters` 已落地（内置 fallback；`LINLIS_ADAPTER_ROOTS` 扫 `*.adapter.json`）。下一刀 P0.1 OpenCode 随包 json。
