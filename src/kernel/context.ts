//! 前端插件上下文：服务容器 + 依赖声明 + 副作用寄存。
//!
//! 设计约束（与 Cordis 语义对齐，和 Rust 侧 `kernel::context` 同构）：
//! - 服务占据一个稳定 key，使用方按 key 取回，不导入具体实现；
//! - 插件用 `inject` 声明依赖，缺失时一次性拿到全部缺口；
//! - 所有注册都登记 disposer，`dispose()` 时按逆序回收；
//! - 释放后的上下文拒绝新注册（fail-closed）。

import { EventBus, type EventHandler, type Subscription } from "./events";

export type KernelErrorCode = "duplicate" | "not_found" | "missing" | "disposed";

/** 服务注册表 / 依赖错误。 */
export class KernelError extends Error {
  readonly code: KernelErrorCode;
  readonly keys: string[];

  constructor(code: KernelErrorCode, message: string, keys: string[] = []) {
    super(message);
    this.name = "KernelError";
    this.code = code;
    this.keys = keys;
  }
}

export interface EffectHandle {
  readonly id: number;
  readonly released: boolean;
}

interface EffectEntry {
  id: number;
  dispose: () => void;
  released: boolean;
}

/** 可克隆的服务容器；克隆共享同一份注册表与副作用栈。 */
export class Context {
  private readonly services = new Map<string, unknown>();
  private readonly effects: EffectEntry[] = [];
  private readonly bus = new EventBus();
  private nextEffectId = 1;
  private disposed = false;

  constructor(private readonly parent: Context | null = null) {}

  /**
   * 派生子作用域：继承父服务（查找时逐级向上），但拥有独立的注册表、
   * 副作用栈与事件总线；`dispose()` 只回收本作用域。
   */
  fork(): Context {
    return new Context(this);
  }

  isDisposed(): boolean {
    return this.disposed;
  }

  events(): EventBus {
    return this.bus;
  }

  /** 注册服务；同 key 重复注册会被拒绝。 */
  provide<T>(key: string, value: T): void {
    if (this.disposed) {
      throw new KernelError("disposed", "上下文已释放");
    }
    if (this.services.has(key)) {
      throw new KernelError("duplicate", `服务已注册：${key}`, [key]);
    }
    this.services.set(key, value);
    this.effects.push({
      id: this.nextEffectId++,
      released: false,
      dispose: () => {
        this.services.delete(key);
      },
    });
  }

  /** 按 key 取回服务；会沿父作用域向上查找。 */
  require<T>(key: string): T {
    if (this.disposed) {
      throw new KernelError("disposed", "上下文已释放");
    }
    const entry = this.lookup(key);
    if (!entry) {
      throw new KernelError("not_found", `服务不存在：${key}`, [key]);
    }
    return entry.value as T;
  }

  has(key: string): boolean {
    return this.lookup(key) !== null;
  }

  /** 返回缺失的 key 列表（不抛错）。 */
  missing(keys: readonly string[]): string[] {
    return keys.filter((key) => this.lookup(key) === null);
  }

  /** 自身 → 父 → 祖父……逐级查找服务。 */
  private lookup(key: string): { value: unknown } | null {
    if (this.services.has(key)) {
      return { value: this.services.get(key) };
    }
    return this.parent ? this.parent.lookup(key) : null;
  }

  /** 校验依赖是否全部就绪；缺失时一次性返回全部缺口。 */
  inject(keys: readonly string[]): void {
    if (this.disposed) {
      throw new KernelError("disposed", "上下文已释放");
    }
    const missing = this.missing(keys);
    if (missing.length > 0) {
      throw new KernelError("missing", `缺少依赖：${missing.join(", ")}`, missing);
    }
  }

  /** 登记一个副作用，`dispose()` 时按逆序执行一次。 */
  effect(dispose: () => void): EffectHandle {
    if (this.disposed) {
      throw new KernelError("disposed", "上下文已释放");
    }
    const entry: EffectEntry = {
      id: this.nextEffectId++,
      dispose,
      released: false,
    };
    this.effects.push(entry);
    return {
      get id() {
        return entry.id;
      },
      get released() {
        return entry.released;
      },
    };
  }

  on(name: string, handler: EventHandler): Subscription {
    return this.bus.on(name, handler);
  }

  /** 逆序回收全部副作用，随后清空服务与监听器；重复调用无副作用。 */
  dispose(): void {
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    for (let index = this.effects.length - 1; index >= 0; index -= 1) {
      const entry = this.effects[index];
      if (entry.released) {
        continue;
      }
      entry.released = true;
      try {
        entry.dispose();
      } catch {
        // 单个清理失败不阻断其余清理，语义与 Rust 侧一致。
      }
    }
    this.effects.length = 0;
    this.services.clear();
    this.bus.clear();
  }
}
