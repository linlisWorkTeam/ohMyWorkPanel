//! 内置插件与进程级内核装配。

pub mod extension_host;

use std::sync::{Arc, OnceLock};

use crate::kernel::{
    start_plugins, Context, LoadReport, ManifestError, PluginManifest, PluginRegistry,
};

/// 注册全部内置插件的实现。
pub fn registry() -> PluginRegistry {
    let mut registry = PluginRegistry::new();
    extension_host::register(&mut registry);
    registry
}

/// 内置清单列表。后续可从磁盘发现更多插件并与内置清单合并。
pub fn builtin_manifests() -> Vec<PluginManifest> {
    vec![extension_host::manifest()]
}

/// 装载内置插件；返回上下文与装载报告。
pub fn bootstrap() -> Result<(Context, LoadReport), ManifestError> {
    let context = Context::new();
    let report = start_plugins(&context, &registry(), builtin_manifests())?;
    Ok((context, report))
}

static KERNEL: OnceLock<Context> = OnceLock::new();

/// 进程级内核。
///
/// v0 用全局单例：桌面与 Web 两个运行时共享同一套服务，避免把 Context 塞进
/// 两套 `AppState`。按群作用域 fork 子上下文留给后续切片。
pub fn kernel() -> &'static Context {
    KERNEL.get_or_init(|| match bootstrap() {
        Ok((context, _report)) => context,
        Err(error) => {
            eprintln!("[kernel] 内置插件装载失败，退回空上下文：{error}");
            Context::new()
        }
    })
}

/// 扩展宿主服务；未装载时返回 `None`，调用方可退回自由函数实现。
pub fn extension_host_service() -> Option<Arc<extension_host::ExtensionHost>> {
    kernel().require(extension_host::SERVICE_KEY).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_loads_builtin_plugins() {
        let (ctx, report) = bootstrap().unwrap();
        assert_eq!(report.loaded, vec![extension_host::PLUGIN_ID]);
        assert!(ctx
            .require::<extension_host::ExtensionHost>(extension_host::SERVICE_KEY)
            .is_ok());
    }

    #[test]
    fn process_kernel_is_initialized_lazily_and_stable() {
        let first = kernel();
        let second = kernel();
        assert!(std::ptr::eq(first, second));
        assert!(extension_host_service().is_some());
    }
}
