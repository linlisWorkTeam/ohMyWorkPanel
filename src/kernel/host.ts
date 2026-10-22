//! 前端内核宿主：装配内置插件，并对装载失败做 fail-open 降级。
//!
//! 与 Rust 侧 `plugins::bootstrap` 同构：装载失败只降级（空上下文），
//! 不允许因为某个前台插件坏掉而白屏。

import { Context, loadPlugins } from "./index";
import { builtinFrontendPlugins } from "./builtins";
import type { PluginSpec } from "./plugin";

let cached: Context | null = null;

/**
 * 装载一组前端插件，返回新上下文。
 *
 * 任一插件失败时返回空上下文（fail-open），调用方按「能力缺失」处理。
 */
export function createFrontendKernel(specs: readonly PluginSpec[] = builtinFrontendPlugins()): Context {
  const ctx = new Context();
  try {
    const report = loadPlugins(ctx, specs);
    console.debug(`[kernel] 前端插件已装载：${report.loaded.join(", ")}`);
    return ctx;
  } catch (error) {
    console.warn("[kernel] 前端内核装载失败，按 fail-open 降级", error);
    return new Context();
  }
}

/** 进程内共享的前端内核（首次调用时装载）。 */
export function frontendKernel(): Context {
  cached ??= createFrontendKernel();
  return cached;
}

/** 重置缓存的宿主内核（测试 / 热更新用）。 */
export function resetFrontendKernel(): void {
  cached = null;
}
