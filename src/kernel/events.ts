//! 前端事件总线：与 Rust 侧 `kernel::events` 语义对齐。
//!
//! | 模式 | 是否等待 | 顺序 | 返回值 |
//! |---|---|---|---|
//! | `emit` | 是 | 注册顺序依次等待 | 被调用数量 |
//! | `parallel` | 是 | 并发扇出 | 每个监听器的可选结果 |
//! | `serial` | 是 | 注册顺序依次等待 | 每个监听器的可选结果 |
//! | `bail` | 是 | 注册顺序，遇到首个非 undefined 即停 | 首个结果 |
//! | `waterfall` | 是 | 注册顺序，值链式传递 | 最终值 |
//!
//! `waterfall`（环绕中间件）暂未实现，与 Rust 侧保持一致，留到事件语义 v0.1。

export type EventHandler = (payload: unknown) => unknown | Promise<unknown>;

export interface Subscription {
  readonly id: number;
}

interface Entry {
  id: number;
  handler: EventHandler;
}

/** 按事件名分组的监听器表。 */
export class EventBus {
  private handlers = new Map<string, Entry[]>();
  private nextId = 1;

  on(name: string, handler: EventHandler): Subscription {
    const entry = { id: this.nextId++, handler };
    const list = this.handlers.get(name) ?? [];
    list.push(entry);
    this.handlers.set(name, list);
    return { id: entry.id };
  }

  off(subscription: Subscription): boolean {
    for (const [name, list] of this.handlers) {
      const index = list.findIndex((entry) => entry.id === subscription.id);
      if (index >= 0) {
        list.splice(index, 1);
        if (list.length === 0) {
          this.handlers.delete(name);
        }
        return true;
      }
    }
    return false;
  }

  handlerCount(name: string): number {
    return this.handlers.get(name)?.length ?? 0;
  }

  clear(): void {
    this.handlers.clear();
  }

  async emit(name: string, payload: unknown): Promise<number> {
    const list = this.snapshot(name);
    for (const entry of list) {
      await entry.handler(payload);
    }
    return list.length;
  }

  async parallel(name: string, payload: unknown): Promise<Array<unknown | undefined>> {
    return Promise.all(this.snapshot(name).map((entry) => entry.handler(payload)));
  }

  async serial(name: string, payload: unknown): Promise<Array<unknown | undefined>> {
    const results: Array<unknown | undefined> = [];
    for (const entry of this.snapshot(name)) {
      results.push(await entry.handler(payload));
    }
    return results;
  }

  async bail(name: string, payload: unknown): Promise<unknown | undefined> {
    for (const entry of this.snapshot(name)) {
      const result = await entry.handler(payload);
      if (result !== undefined) {
        return result;
      }
    }
    return undefined;
  }

  /**
   * 环绕中间件（waterfall）：上一个监听器的返回值作为下一个的入参。
   * 返回 `undefined` 表示「不修改」，当前值继续向下传递。
   */
  async waterfall(name: string, payload: unknown): Promise<unknown> {
    let current = payload;
    for (const entry of this.snapshot(name)) {
      const next = await entry.handler(current);
      if (next !== undefined) {
        current = next;
      }
    }
    return current;
  }

  private snapshot(name: string): Entry[] {
    return [...(this.handlers.get(name) ?? [])];
  }
}
