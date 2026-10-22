//! 插件装载：把一组声明了依赖与产出的插件按依赖顺序装进上下文。
//!
//! v0 边界（与 Rust 侧 `kernel::plugin` 一致，刻意保守）：
//! - 插件是编译期注册的对象，不做动态加载；
//! - 装载是同步的，依赖必须先于消费者就绪；
//! - 任一插件安装失败即整体回滚（释放上下文），不允许"半套插件"继续跑。

import { Context, KernelError } from "./context";

export interface PluginSpec {
  /** 全局唯一 id。 */
  readonly id: string;
  /** 安装后应出现的服务 key；装载器会校验，避免"声明了却没注册"。 */
  readonly provides: readonly string[];
  /** 安装前必须就绪的服务 key。 */
  readonly inject: readonly string[];
  /** 安装函数；抛错即视为安装失败。 */
  readonly install: (ctx: Context) => void;
}

export type PluginLoadErrorKind =
  | "duplicate_id"
  | "missing_dependency"
  | "cycle"
  | "provides_not_registered"
  | "install";

export class PluginLoadError extends Error {
  readonly kind: PluginLoadErrorKind;
  readonly plugin?: string;
  readonly missing?: string[];
  readonly ids?: string[];

  constructor(
    kind: PluginLoadErrorKind,
    message: string,
    detail: { plugin?: string; missing?: string[]; ids?: string[] } = {},
  ) {
    super(message);
    this.name = "PluginLoadError";
    this.kind = kind;
    this.plugin = detail.plugin;
    this.missing = detail.missing;
    this.ids = detail.ids;
  }
}

export interface LoadReport {
  /** 按实际安装顺序返回插件 id。 */
  readonly loaded: string[];
}

/**
 * 按依赖顺序装载插件。
 *
 * 失败时上下文会被释放（fail-closed），调用方不应继续使用它。
 */
export function loadPlugins(ctx: Context, specs: readonly PluginSpec[]): LoadReport {
  const seen = new Set<string>();
  for (const spec of specs) {
    if (seen.has(spec.id)) {
      throw new PluginLoadError("duplicate_id", `插件 id 重复：${spec.id}`, { plugin: spec.id });
    }
    seen.add(spec.id);
  }

  const pending = [...specs];
  const loaded: string[] = [];

  while (pending.length > 0) {
    let progressed = false;
    let index = 0;
    while (index < pending.length) {
      const spec = pending[index];
      let ready = true;
      try {
        ctx.inject(spec.inject);
      } catch (error) {
        if (error instanceof KernelError && error.code === "missing") {
          ready = false;
        } else {
          ctx.dispose();
          throw new PluginLoadError("install", messageOf(error), { plugin: spec.id });
        }
      }
      if (!ready) {
        index += 1;
        continue;
      }

      pending.splice(index, 1);
      try {
        spec.install(ctx);
      } catch (error) {
        ctx.dispose();
        throw new PluginLoadError("install", messageOf(error), { plugin: spec.id });
      }
      const missing = ctx.missing(spec.provides);
      if (missing.length > 0) {
        ctx.dispose();
        throw new PluginLoadError(
          "provides_not_registered",
          `插件 ${spec.id} 声明提供但未注册：${missing.join(", ")}`,
          { plugin: spec.id, missing },
        );
      }
      loaded.push(spec.id);
      progressed = true;
    }

    if (!progressed) {
      throw stallError(ctx, pending);
    }
  }

  return { loaded };
}

/** 没有任何插件可装载时，区分「真缺依赖」和「互相等待成环」。 */
function stallError(ctx: Context, pending: readonly PluginSpec[]): PluginLoadError {
  const providers = new Map<string, string>();
  for (const spec of pending) {
    for (const key of spec.provides) {
      providers.set(key, spec.id);
    }
  }

  const gaps = pending.map((spec) => ({ id: spec.id, missing: ctx.missing(spec.inject) }));
  const cyclic = gaps.every(
    (gap) => gap.missing.length > 0 && gap.missing.every((key) => providers.has(key)),
  );
  const ids = pending.map((spec) => spec.id);
  ctx.dispose();
  if (cyclic) {
    return new PluginLoadError("cycle", `插件依赖成环：${ids.join(" -> ")}`, { ids });
  }

  const missingGap = [...gaps]
    .sort((a, b) => a.id.localeCompare(b.id))
    .find((gap) => gap.missing.length > 0);
  const plugin = missingGap?.id ?? ids[0] ?? "unknown";
  const missing = missingGap?.missing ?? [];
  return new PluginLoadError("missing_dependency", `插件 ${plugin} 缺少依赖：${missing.join(", ")}`, {
    plugin,
    missing,
  });
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
