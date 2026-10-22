//! 插件上下文：服务容器 + 依赖声明 + 副作用宿主。
//!
//! 设计约束（与 Cordis 语义对齐）：
//! - 服务占据一个稳定的 `ctx.<key>`，使用者按 key 取回，不导入具体实现；
//! - 插件用 `inject` 声明依赖，缺失时一次性拿到全部缺口；
//! - 所有注册都登记 disposer，`dispose()` 时按逆序回收；
//! - 释放后的上下文拒绝新注册（fail-closed），避免"半个插件还在跑"。

use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use super::effect::{EffectHandle, EffectStack};
use super::events::EventBus;

type Service = Arc<dyn Any + Send + Sync>;

/// 服务注册表错误。调用方可以 `map_err(String::from)` 接入 `AppResult`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceRegistryError {
    /// 同一个 key 被注册两次。
    Duplicate(&'static str),
    /// key 不存在。
    NotFound(&'static str),
    /// key 存在但类型不匹配。
    TypeMismatch(&'static str),
    /// 依赖缺失（一次性列出全部）。
    Missing(Vec<&'static str>),
    /// 上下文已释放。
    Disposed,
}

impl fmt::Display for ServiceRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(key) => write!(f, "服务已注册：{key}"),
            Self::NotFound(key) => write!(f, "服务不存在：{key}"),
            Self::TypeMismatch(key) => write!(f, "服务类型不匹配：{key}"),
            Self::Missing(keys) => write!(f, "缺少依赖：{}", keys.join(", ")),
            Self::Disposed => write!(f, "上下文已释放"),
        }
    }
}

impl std::error::Error for ServiceRegistryError {}

impl From<ServiceRegistryError> for String {
    fn from(error: ServiceRegistryError) -> Self {
        error.to_string()
    }
}

struct ContextInner {
    services: RwLock<HashMap<&'static str, Service>>,
    effects: EffectStack,
    events: EventBus,
    disposed: AtomicBool,
    /// 父作用域；子上下文按 key 逐级向上查找服务，但副作用只回收自己的。
    parent: Option<Arc<ContextInner>>,
}

impl ContextInner {
    /// 自身 → 父 → 祖父……逐级查找服务。
    fn lookup(&self, key: &'static str) -> Option<Service> {
        if let Ok(services) = self.services.read() {
            if let Some(service) = services.get(key) {
                return Some(Arc::clone(service));
            }
        }
        self.parent.as_ref().and_then(|parent| parent.lookup(key))
    }
}

/// 可克隆的服务容器；克隆共享同一份注册表与副作用栈。
#[derive(Clone)]
pub struct Context {
    inner: Arc<ContextInner>,
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ContextInner {
                services: RwLock::new(HashMap::new()),
                effects: EffectStack::default(),
                events: EventBus::new(),
                disposed: AtomicBool::new(false),
                parent: None,
            }),
        }
    }

    /// 派生子作用域：继承父服务（查找时逐级向上），但拥有独立的注册表、
    /// 副作用栈与事件总线；`dispose()` 只回收本作用域。
    pub fn fork(&self) -> Context {
        Self {
            inner: Arc::new(ContextInner {
                services: RwLock::new(HashMap::new()),
                effects: EffectStack::default(),
                events: EventBus::new(),
                disposed: AtomicBool::new(false),
                parent: Some(Arc::clone(&self.inner)),
            }),
        }
    }

    pub fn is_disposed(&self) -> bool {
        self.inner.disposed.load(Ordering::SeqCst)
    }

    fn ensure_alive(&self) -> Result<(), ServiceRegistryError> {
        if self.is_disposed() {
            Err(ServiceRegistryError::Disposed)
        } else {
            Ok(())
        }
    }

    /// 注册服务。返回的句柄便于观察回收；实际释放由 [`Context::dispose`] 驱动。
    pub fn provide<T>(
        &self,
        key: &'static str,
        value: T,
    ) -> Result<Arc<EffectHandle>, ServiceRegistryError>
    where
        T: Any + Send + Sync,
    {
        self.ensure_alive()?;
        {
            let mut services = self
                .inner
                .services
                .write()
                .map_err(|_| ServiceRegistryError::Disposed)?;
            if services.contains_key(key) {
                return Err(ServiceRegistryError::Duplicate(key));
            }
            services.insert(key, Arc::new(value) as Service);
        }
        let inner = Arc::clone(&self.inner);
        Ok(self.inner.effects.push(Box::new(move || {
            if let Ok(mut services) = inner.services.write() {
                services.remove(key);
            }
        })))
    }

    /// 按 key 取回服务并做类型校验；会沿父作用域向上查找。
    pub fn require<T>(&self, key: &'static str) -> Result<Arc<T>, ServiceRegistryError>
    where
        T: Any + Send + Sync,
    {
        let service = self
            .inner
            .lookup(key)
            .ok_or(ServiceRegistryError::NotFound(key))?;
        service
            .downcast::<T>()
            .map_err(|_| ServiceRegistryError::TypeMismatch(key))
    }

    /// 列出全部缺失依赖（用于启动时报出完整缺口，而不是逐个试错）。
    pub fn missing(&self, keys: &[&'static str]) -> Vec<&'static str> {
        keys.iter()
            .copied()
            .filter(|key| self.inner.lookup(key).is_none())
            .collect()
    }

    /// 声明依赖：全部就绪才返回 `Ok`，否则一次性返回缺失项。
    pub fn inject(&self, keys: &[&'static str]) -> Result<(), ServiceRegistryError> {
        let missing = self.missing(keys);
        if missing.is_empty() {
            Ok(())
        } else {
            Err(ServiceRegistryError::Missing(missing))
        }
    }

    /// 事件总线；上下文释放时监听器会被清空。
    pub fn events(&self) -> &EventBus {
        &self.inner.events
    }

    /// 登记一个可撤销副作用。
    pub fn effect(
        &self,
        dispose: impl FnOnce() + Send + 'static,
    ) -> Result<Arc<EffectHandle>, ServiceRegistryError> {
        self.ensure_alive()?;
        Ok(self.inner.effects.push(Box::new(dispose)))
    }

    /// 释放上下文：标记已释放，并按逆序回收全部副作用。重复调用安全。
    pub fn dispose(&self) {
        if self.inner.disposed.swap(true, Ordering::SeqCst) {
            return;
        }
        self.inner.effects.dispose_all();
        self.inner.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[derive(Debug)]
    struct Greeter(&'static str);

    #[test]
    fn provide_and_require_are_typed() {
        let ctx = Context::new();
        ctx.provide("greeter", Greeter("hello")).unwrap();
        let greeter = ctx.require::<Greeter>("greeter").unwrap();
        assert_eq!(greeter.0, "hello");
    }

    #[test]
    fn duplicate_registration_is_rejected() {
        let ctx = Context::new();
        ctx.provide("greeter", Greeter("a")).unwrap();
        let error = ctx.provide("greeter", Greeter("b")).unwrap_err();
        assert_eq!(error, ServiceRegistryError::Duplicate("greeter"));
    }

    #[test]
    fn missing_dependencies_are_reported_together() {
        let ctx = Context::new();
        ctx.provide("a", Greeter("a")).unwrap();
        let error = ctx.inject(&["a", "b", "c"]).unwrap_err();
        assert_eq!(error, ServiceRegistryError::Missing(vec!["b", "c"]));
    }

    #[test]
    fn type_mismatch_is_reported() {
        let ctx = Context::new();
        ctx.provide("greeter", Greeter("a")).unwrap();
        let error = ctx.require::<u32>("greeter").unwrap_err();
        assert_eq!(error, ServiceRegistryError::TypeMismatch("greeter"));
    }

    #[test]
    fn effects_are_disposed_in_reverse_order_once() {
        let ctx = Context::new();
        let order = Arc::new(std::sync::Mutex::new(Vec::new()));
        let first = Arc::clone(&order);
        let second = Arc::clone(&order);
        let handle_one = ctx.effect(move || first.lock().unwrap().push(1)).unwrap();
        let handle_two = ctx.effect(move || second.lock().unwrap().push(2)).unwrap();

        ctx.dispose();
        ctx.dispose();

        assert_eq!(*order.lock().unwrap(), vec![2, 1]);
        assert!(handle_one.is_released());
        assert!(handle_two.is_released());
    }

    #[test]
    fn disposed_context_rejects_registration_and_drops_services() {
        let ctx = Context::new();
        ctx.provide("greeter", Greeter("a")).unwrap();
        ctx.dispose();

        assert_eq!(
            ctx.effect(|| {}).unwrap_err(),
            ServiceRegistryError::Disposed
        );
        assert_eq!(
            ctx.provide("other", Greeter("b")).unwrap_err(),
            ServiceRegistryError::Disposed
        );
        assert_eq!(
            ctx.require::<Greeter>("greeter").unwrap_err(),
            ServiceRegistryError::NotFound("greeter")
        );
    }

    #[test]
    fn effects_run_exactly_once_under_shared_context() {
        let ctx = Context::new();
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&runs);
        ctx.effect(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();

        let clone = ctx.clone();
        clone.dispose();
        ctx.dispose();

        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn fork_inherits_parent_services() {
        let root = Context::new();
        root.provide("greeter", Greeter("parent")).unwrap();
        let child = root.fork();

        let greeter = child.require::<Greeter>("greeter").unwrap();
        assert_eq!(greeter.0, "parent");
        assert!(child.inject(&["greeter"]).is_ok());
    }

    #[test]
    fn fork_may_shadow_parent_service() {
        let root = Context::new();
        root.provide("greeter", Greeter("parent")).unwrap();
        let child = root.fork();
        child.provide("greeter", Greeter("child")).unwrap();

        assert_eq!(child.require::<Greeter>("greeter").unwrap().0, "child");
        assert_eq!(root.require::<Greeter>("greeter").unwrap().0, "parent");
    }

    #[test]
    fn fork_dispose_only_releases_its_own_effects() {
        let root = Context::new();
        root.provide("greeter", Greeter("parent")).unwrap();
        let order = Arc::new(std::sync::Mutex::new(Vec::new()));
        let child = root.fork();
        let buffer = Arc::clone(&order);
        child
            .effect(move || buffer.lock().unwrap().push("child"))
            .unwrap();

        child.dispose();

        assert!(child.is_disposed());
        assert_eq!(*order.lock().unwrap(), vec!["child"]);
        assert_eq!(root.require::<Greeter>("greeter").unwrap().0, "parent");
    }

    #[test]
    fn disposed_parent_no_longer_serves_children() {
        let root = Context::new();
        root.provide("greeter", Greeter("parent")).unwrap();
        let child = root.fork();

        root.dispose();

        assert_eq!(
            child.require::<Greeter>("greeter").unwrap_err(),
            ServiceRegistryError::NotFound("greeter")
        );
        assert_eq!(child.missing(&["greeter"]), vec!["greeter"]);
    }
}
