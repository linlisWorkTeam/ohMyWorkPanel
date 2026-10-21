//! 类型化事件总线：四种分发语义。
//!
//! | 模式 | 是否等待 | 顺序 | 返回值 |
//! |---|---|---|---|
//! | `emit` | 是 | 注册顺序依次等待 | 无（只返回被调用数量） |
//! | `parallel` | 是 | 并发扇出 | 每个监听器的可选结果 |
//! | `serial` | 是 | 注册顺序依次等待 | 每个监听器的可选结果 |
//! | `bail` | 是 | 注册顺序，遇到首个返回值即停 | 首个返回值 |
//!
//! 与 Cordis 的差异：`waterfall`（环绕中间件）暂未实现，留到事件语义 v0.1。
//! 监听器统一以 `serde_json::Value` 作为载荷，避免为每种事件写一套泛型。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use futures_util::future::{join_all, BoxFuture};
use serde_json::Value;

/// 异步事件监听器：接收载荷，返回可选结果（`emit` 会忽略结果）。
pub type EventHandler = Arc<dyn Fn(Value) -> BoxFuture<'static, Option<Value>> + Send + Sync>;

/// 分发模式，用于在规格与调用点之间对齐预期。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchMode {
    Emit,
    Parallel,
    Serial,
    Bail,
}

/// 监听器注册句柄；`off` 时使用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subscription {
    id: u64,
}

impl Subscription {
    pub fn id(self) -> u64 {
        self.id
    }
}

/// 按事件名分组的监听器表；克隆上下文共享同一张表。
#[derive(Default)]
pub struct EventBus {
    handlers: RwLock<HashMap<&'static str, Vec<(u64, EventHandler)>>>,
    next_id: AtomicU64,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册异步监听器。
    pub fn on(&self, name: &'static str, handler: EventHandler) -> Subscription {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        self.handlers
            .write()
            .expect("event registry lock poisoned")
            .entry(name)
            .or_default()
            .push((id, handler));
        Subscription { id }
    }

    /// 注册同步监听器（内部包装为立即完成的 future）。
    pub fn on_sync<F>(&self, name: &'static str, handler: F) -> Subscription
    where
        F: Fn(Value) -> Option<Value> + Send + Sync + 'static,
    {
        self.on(
            name,
            Arc::new(move |payload: Value| -> BoxFuture<'static, Option<Value>> {
                let result = handler(payload);
                Box::pin(async move { result })
            }),
        )
    }

    /// 注销监听器；返回是否找到。
    pub fn off(&self, subscription: Subscription) -> bool {
        let mut handlers = self.handlers.write().expect("event registry lock poisoned");
        let mut removed = false;
        for entries in handlers.values_mut() {
            let before = entries.len();
            entries.retain(|(id, _)| *id != subscription.id);
            removed |= entries.len() != before;
        }
        removed
    }

    pub fn handler_count(&self, name: &'static str) -> usize {
        self.handlers
            .read()
            .expect("event registry lock poisoned")
            .get(name)
            .map_or(0, Vec::len)
    }

    /// 清空全部监听器（上下文释放时调用）。
    pub fn clear(&self) {
        self.handlers
            .write()
            .expect("event registry lock poisoned")
            .clear();
    }

    fn snapshot(&self, name: &'static str) -> Vec<EventHandler> {
        self.handlers
            .read()
            .expect("event registry lock poisoned")
            .get(name)
            .map(|entries| {
                entries
                    .iter()
                    .map(|(_, handler)| Arc::clone(handler))
                    .collect::<Vec<EventHandler>>()
            })
            .unwrap_or_default()
    }

    /// 依次等待每个监听器，忽略返回值；返回被调用的监听器数量。
    pub async fn emit(&self, name: &'static str, payload: Value) -> usize {
        let handlers = self.snapshot(name);
        let count = handlers.len();
        for handler in handlers {
            let _ = handler(payload.clone()).await;
        }
        count
    }

    /// 并发扇出，收集每个监听器的结果。
    pub async fn parallel(&self, name: &'static str, payload: Value) -> Vec<Option<Value>> {
        let handlers = self.snapshot(name);
        let futures = handlers.into_iter().map(|handler| handler(payload.clone()));
        join_all(futures).await
    }

    /// 串行等待，按注册顺序收集结果。
    pub async fn serial(&self, name: &'static str, payload: Value) -> Vec<Option<Value>> {
        let handlers = self.snapshot(name);
        let mut results = Vec::with_capacity(handlers.len());
        for handler in handlers {
            results.push(handler(payload.clone()).await);
        }
        results
    }

    /// 按注册顺序调用，遇到首个 `Some` 立即返回并停止后续监听器。
    pub async fn bail(&self, name: &'static str, payload: Value) -> Option<Value> {
        let handlers = self.snapshot(name);
        for handler in handlers {
            if let Some(value) = handler(payload.clone()).await {
                return Some(value);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    #[tokio::test]
    async fn emit_invokes_handlers_in_registration_order() {
        let bus = EventBus::new();
        let seen = Arc::new(Mutex::new(Vec::new()));
        for tag in ["a", "b", "c"] {
            let seen = Arc::clone(&seen);
            bus.on_sync("demo", move |_| {
                seen.lock().unwrap().push(tag);
                None
            });
        }

        let called = bus.emit("demo", json!({})).await;

        assert_eq!(called, 3);
        assert_eq!(*seen.lock().unwrap(), vec!["a", "b", "c"]);
    }

    #[tokio::test]
    async fn bail_stops_at_first_returned_value() {
        let bus = EventBus::new();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let first = Arc::clone(&seen);
        bus.on_sync("guard", move |_| {
            first.lock().unwrap().push("first");
            None
        });
        bus.on_sync("guard", |_| Some(json!({"decision": "allow"})));
        let unreachable = Arc::clone(&seen);
        bus.on_sync("guard", move |_| {
            unreachable.lock().unwrap().push("unreachable");
            Some(json!({"decision": "deny"}))
        });

        let decision = bus.bail("guard", json!({})).await;

        assert_eq!(decision, Some(json!({"decision": "allow"})));
        assert_eq!(*seen.lock().unwrap(), vec!["first"]);
    }

    #[tokio::test]
    async fn serial_collects_results_in_registration_order() {
        let bus = EventBus::new();
        bus.on_sync("chain", |_| Some(json!(1)));
        bus.on_sync("chain", |_| Some(json!(2)));

        let results = bus.serial("chain", json!({})).await;

        assert_eq!(results, vec![Some(json!(1)), Some(json!(2))]);
    }

    #[tokio::test]
    async fn parallel_collects_every_result() {
        let bus = EventBus::new();
        bus.on_sync("fan", |_| Some(json!("x")));
        bus.on_sync("fan", |_| Some(json!("y")));

        let mut results = bus.parallel("fan", json!({})).await;
        results.sort_by_key(|value| value.as_ref().and_then(Value::as_str).map(str::to_string));

        assert_eq!(results, vec![Some(json!("x")), Some(json!("y"))]);
    }

    #[tokio::test]
    async fn off_removes_only_the_target_subscription() {
        let bus = EventBus::new();
        let keep = bus.on_sync("demo", |_| None);
        let drop_me = bus.on_sync("demo", |_| None);
        assert_eq!(bus.handler_count("demo"), 2);

        assert!(bus.off(drop_me));

        assert_eq!(bus.handler_count("demo"), 1);
        assert!(!bus.off(drop_me));
        bus.emit("demo", json!({})).await;
        assert!(keep.id() > 0);
    }
}
