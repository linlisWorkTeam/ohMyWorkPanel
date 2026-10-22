//! 前端内置插件清单。
//!
//! 与 Rust 侧 `plugins::builtin_manifests` 对应：这里列出随壳编译期注册的前端插件。

import type { PluginSpec } from "./index";
import { scenePlugin } from "../scene/plugin";

/** 前端内置插件。 */
export function builtinFrontendPlugins(): PluginSpec[] {
  return [scenePlugin()];
}
