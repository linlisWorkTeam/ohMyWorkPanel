//! 插件装载：把一组声明了依赖与产出的插件按依赖顺序装进上下文。
//!
//! v0 的边界（刻意保守）：
//! - 插件是**编译期注册**的闭包，不做动态库加载；
//! - 装载是同步的，依赖必须先于消费者就绪；
//! - 任一插件安装失败即整体回滚（释放上下文），不允许"半套插件"继续跑。

use std::collections::{HashMap, HashSet};
use std::fmt;

use super::context::{Context, ServiceRegistryError};

/// 插件安装函数：注册服务、事件与副作用。用 `Arc` 以便注册表复用同一个安装函数。
pub type InstallFn = std::sync::Arc<dyn Fn(&Context) -> Result<(), String> + Send + Sync>;

/// 一个可装载插件的静态描述。
pub struct PluginSpec {
    /// 全局唯一 id。
    pub id: &'static str,
    /// 安装后应出现的服务 key；装载器会校验，避免"声明了却没注册"。
    pub provides: Vec<&'static str>,
    /// 安装前必须就绪的服务 key。
    pub inject: Vec<&'static str>,
    /// 安装函数。
    pub install: InstallFn,
}

impl PluginSpec {
    pub fn new(
        id: &'static str,
        provides: Vec<&'static str>,
        inject: Vec<&'static str>,
        install: impl Fn(&Context) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            id,
            provides,
            inject,
            install: std::sync::Arc::new(install),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginLoadError {
    /// 插件 id 重复。
    DuplicateId(&'static str),
    /// 依赖缺失，且没有其他待装载插件能提供。
    MissingDependency {
        plugin: &'static str,
        missing: Vec<&'static str>,
    },
    /// 依赖成环，无法确定顺序。
    Cycle(Vec<&'static str>),
    /// 插件声明会提供服务，但安装后并没有注册。
    ProvidesNotRegistered {
        plugin: &'static str,
        missing: Vec<&'static str>,
    },
    /// 插件安装函数返回错误。
    Install { plugin: &'static str, error: String },
}

impl fmt::Display for PluginLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "插件 id 重复：{id}"),
            Self::MissingDependency { plugin, missing } => {
                write!(f, "插件 {plugin} 缺少依赖：{}", missing.join(", "))
            }
            Self::Cycle(ids) => write!(f, "插件依赖成环：{}", ids.join(" -> ")),
            Self::ProvidesNotRegistered { plugin, missing } => {
                write!(f, "插件 {plugin} 声明提供但未注册：{}", missing.join(", "))
            }
            Self::Install { plugin, error } => write!(f, "插件 {plugin} 安装失败：{error}"),
        }
    }
}

impl std::error::Error for PluginLoadError {}

impl From<PluginLoadError> for String {
    fn from(error: PluginLoadError) -> Self {
        error.to_string()
    }
}

/// 装载结果：按实际安装顺序返回插件 id。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadReport {
    pub loaded: Vec<&'static str>,
}

/// 按依赖顺序装载插件。
///
/// 失败时上下文会被释放（fail-closed），调用方不应继续使用它。
pub fn load_plugins(ctx: &Context, specs: Vec<PluginSpec>) -> Result<LoadReport, PluginLoadError> {
    let mut seen = HashSet::new();
    for spec in &specs {
        if !seen.insert(spec.id) {
            return Err(PluginLoadError::DuplicateId(spec.id));
        }
    }

    let mut pending = specs;
    let mut loaded: Vec<&'static str> = Vec::new();

    while !pending.is_empty() {
        let mut progressed = false;
        let mut index = 0;
        while index < pending.len() {
            let ready = match ctx.inject(&pending[index].inject) {
                Ok(()) => true,
                Err(ServiceRegistryError::Missing(_)) => false,
                Err(error) => {
                    ctx.dispose();
                    return Err(PluginLoadError::Install {
                        plugin: pending[index].id,
                        error: error.to_string(),
                    });
                }
            };
            if !ready {
                index += 1;
                continue;
            }

            let spec = pending.remove(index);
            if let Err(error) = (spec.install)(ctx) {
                ctx.dispose();
                return Err(PluginLoadError::Install {
                    plugin: spec.id,
                    error,
                });
            }
            if let Err(ServiceRegistryError::Missing(missing)) = ctx.inject(&spec.provides) {
                ctx.dispose();
                return Err(PluginLoadError::ProvidesNotRegistered {
                    plugin: spec.id,
                    missing,
                });
            }
            loaded.push(spec.id);
            progressed = true;
        }

        if !progressed {
            return Err(stall_error(ctx, &pending));
        }
    }

    Ok(LoadReport { loaded })
}

/// 没有任何插件可装载时，区分「真缺依赖」和「互相等待成环」。
fn stall_error(ctx: &Context, pending: &[PluginSpec]) -> PluginLoadError {
    let mut providers: HashMap<&'static str, &'static str> = HashMap::new();
    for spec in pending {
        for key in &spec.provides {
            providers.insert(key, spec.id);
        }
    }

    let gaps: Vec<(&'static str, Vec<&'static str>)> = pending
        .iter()
        .map(|spec| (spec.id, ctx.missing(&spec.inject)))
        .collect();
    let cyclic = gaps.iter().all(|(_, missing)| {
        !missing.is_empty() && missing.iter().all(|key| providers.contains_key(key))
    });
    let ids: Vec<&'static str> = pending.iter().map(|spec| spec.id).collect();
    ctx.dispose();
    if cyclic {
        return PluginLoadError::Cycle(ids);
    }

    let mut reported = gaps;
    reported.sort_by_key(|(id, _)| *id);
    let (plugin, missing) = reported
        .into_iter()
        .find(|(_, missing)| !missing.is_empty())
        .unwrap_or((ids.first().copied().unwrap_or("unknown"), Vec::new()));
    PluginLoadError::MissingDependency { plugin, missing }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn recorder() -> Arc<Mutex<Vec<&'static str>>> {
        Arc::new(Mutex::new(Vec::new()))
    }

    #[test]
    fn loads_dependencies_before_consumers() {
        let ctx = Context::new();
        let order = recorder();
        let writer = Arc::clone(&order);

        let provider = PluginSpec::new("provider", vec!["svc.a"], vec![], |ctx| {
            ctx.provide("svc.a", 1_u8).map_err(String::from)?;
            Ok(())
        });
        let consumer = PluginSpec::new("consumer", vec![], vec!["svc.a"], move |_ctx| {
            writer.lock().unwrap().push("consumer");
            Ok(())
        });

        let report = load_plugins(&ctx, vec![consumer, provider]).unwrap();

        assert_eq!(report.loaded, vec!["provider", "consumer"]);
        assert_eq!(*order.lock().unwrap(), vec!["consumer"]);
    }

    #[test]
    fn independent_plugins_keep_input_order() {
        let ctx = Context::new();
        let order = recorder();
        let writer = Arc::clone(&order);
        let first = PluginSpec::new("first", vec![], vec![], {
            let writer = Arc::clone(&writer);
            move |_| {
                writer.lock().unwrap().push("first");
                Ok(())
            }
        });
        let second = PluginSpec::new("second", vec![], vec![], move |_| {
            writer.lock().unwrap().push("second");
            Ok(())
        });

        let report = load_plugins(&ctx, vec![first, second]).unwrap();

        assert_eq!(report.loaded, vec!["first", "second"]);
        assert_eq!(*order.lock().unwrap(), vec!["first", "second"]);
    }

    #[test]
    fn missing_dependency_is_reported() {
        let ctx = Context::new();
        let spec = PluginSpec::new("needy", vec![], vec!["nope"], |_| Ok(()));

        let error = load_plugins(&ctx, vec![spec]).unwrap_err();

        assert_eq!(
            error,
            PluginLoadError::MissingDependency {
                plugin: "needy",
                missing: vec!["nope"]
            }
        );
        assert!(ctx.is_disposed());
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let ctx = Context::new();
        let first = PluginSpec::new("dup", vec![], vec![], |_| Ok(()));
        let second = PluginSpec::new("dup", vec![], vec![], |_| Ok(()));

        let error = load_plugins(&ctx, vec![first, second]).unwrap_err();

        assert_eq!(error, PluginLoadError::DuplicateId("dup"));
    }

    #[test]
    fn install_failure_disposes_context() {
        let ctx = Context::new();
        let spec = PluginSpec::new("boom", vec![], vec![], |_| Err("kaboom".into()));

        let error = load_plugins(&ctx, vec![spec]).unwrap_err();

        assert_eq!(
            error,
            PluginLoadError::Install {
                plugin: "boom",
                error: "kaboom".into()
            }
        );
        assert!(ctx.is_disposed());
    }

    #[test]
    fn declared_but_missing_service_is_rejected() {
        let ctx = Context::new();
        let spec = PluginSpec::new("liar", vec!["svc.missing"], vec![], |_| Ok(()));

        let error = load_plugins(&ctx, vec![spec]).unwrap_err();

        assert_eq!(
            error,
            PluginLoadError::ProvidesNotRegistered {
                plugin: "liar",
                missing: vec!["svc.missing"]
            }
        );
        assert!(ctx.is_disposed());
    }

    #[test]
    fn cyclic_dependencies_are_reported() {
        let ctx = Context::new();
        let a = PluginSpec::new("a", vec!["svc.a"], vec!["svc.b"], |_| Ok(()));
        let b = PluginSpec::new("b", vec!["svc.b"], vec!["svc.a"], |_| Ok(()));

        let error = load_plugins(&ctx, vec![a, b]).unwrap_err();

        assert!(matches!(error, PluginLoadError::Cycle(_)));
        assert!(ctx.is_disposed());
    }
}
