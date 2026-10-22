//! 前端插件内核（P1 TS 同构侧）。
//!
//! 与 Rust 侧 `src-tauri/src/kernel/` 同构：服务容器、依赖注入、可撤销副作用、
//! 类型化事件、按依赖顺序装载与失败回滚。行为契约见 `openspec/specs/kernel/spec.md`。

export { Context, KernelError } from "./context";
export type { EffectHandle, KernelErrorCode } from "./context";
export { EventBus } from "./events";
export type { EventHandler, Subscription } from "./events";
export { loadPlugins, PluginLoadError } from "./plugin";
export type { LoadReport, PluginLoadErrorKind, PluginSpec } from "./plugin";
