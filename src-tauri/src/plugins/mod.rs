//! 内置插件与进程级内核装配。

pub mod adapter_catalog;
pub mod extension_host;
pub mod marketing;
pub mod workflow;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use crate::kernel::{
    discover_manifests, start_plugins, Context, LoadReport, ManifestError, PluginManifest,
    PluginRegistry,
};

/// 注册全部内置插件的实现。
pub fn registry() -> PluginRegistry {
    let mut registry = PluginRegistry::new();
    extension_host::register(&mut registry);
    adapter_catalog::register(&mut registry);
    workflow::register(&mut registry);
    marketing::register(&mut registry);
    registry
}

/// 内置清单列表。后续可从磁盘发现更多插件并与内置清单合并。
pub fn builtin_manifests() -> Vec<PluginManifest> {
    vec![
        extension_host::manifest(),
        adapter_catalog::manifest(),
        workflow::manifest(),
        marketing::manifest(),
    ]
}

/// 装载内置插件；返回上下文与装载报告。
pub fn bootstrap() -> Result<(Context, LoadReport), ManifestError> {
    bootstrap_with_roots(&plugin_roots())
}

/// `OHMYWORKPANEL_PLUGIN_ROOTS`：`:` / `;` 分隔的插件目录列表；未设置则不扫描磁盘。
pub fn plugin_roots() -> Vec<PathBuf> {
    std::env::var("OHMYWORKPANEL_PLUGIN_ROOTS")
        .map(|raw| {
            raw.split([':', ';'])
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}

/// 内置清单 + 磁盘发现的清单。
///
/// 磁盘清单只有在 `entry` 已注册时才纳入；未知入口与损坏清单只告警，不影响内置插件。
pub fn collect_manifests(registry: &PluginRegistry, roots: &[PathBuf]) -> Vec<PluginManifest> {
    let mut manifests = builtin_manifests();
    if roots.is_empty() {
        return manifests;
    }
    for discovery in discover_manifests(roots) {
        match discovery.result {
            Ok(manifest) if registry.contains(&manifest.entry) => manifests.push(manifest),
            Ok(manifest) => eprintln!(
                "[kernel] 跳过未知入口的插件清单：{}（entry={}）",
                manifest.id, manifest.entry
            ),
            Err(error) => eprintln!(
                "[kernel] 插件清单无效（{}）：{error}",
                discovery.root.display()
            ),
        }
    }
    manifests
}

/// 带磁盘扫描的装载入口（测试与启动共用）。
pub fn bootstrap_with_roots(roots: &[PathBuf]) -> Result<(Context, LoadReport), ManifestError> {
    let registry = registry();
    let context = Context::new();
    let report = start_plugins(&context, &registry, collect_manifests(&registry, roots))?;
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

/// 适配器目录服务；未装载时返回 `None`，调用方可退回自由函数实现。
pub fn adapter_catalog_service() -> Option<Arc<adapter_catalog::AdapterCatalog>> {
    kernel().require(adapter_catalog::SERVICE_KEY).ok()
}

/// 工作流服务；未装载时返回 `None`，调用方可退回自由函数实现。
pub fn workflow_service() -> Option<Arc<workflow::WorkflowService>> {
    kernel().require(workflow::SERVICE_KEY).ok()
}

/// Marketing 查询服务；未装载时返回 `None`，调用方可退回自由函数实现。
pub fn marketing_service() -> Option<Arc<marketing::MarketingService>> {
    kernel().require(marketing::SERVICE_KEY).ok()
}

/// 取工作流服务；内核未装载时返回明确错误（传输层映射为 503）。
///
/// Web 入口只走服务，不再保留自由函数回退——避免「两套实现」长期漂移。
pub fn require_workflow_service() -> Result<Arc<workflow::WorkflowService>, String> {
    workflow_service().ok_or_else(|| "插件内核未装载：svc.workflow".to_string())
}

/// 取 Marketing 服务；内核未装载时返回明确错误（传输层映射为 503）。
pub fn require_marketing_service() -> Result<Arc<marketing::MarketingService>, String> {
    marketing_service().ok_or_else(|| "插件内核未装载：svc.marketing".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_loads_builtin_plugins() {
        let (ctx, report) = bootstrap().unwrap();
        assert_eq!(
            report.loaded,
            vec![
                extension_host::PLUGIN_ID,
                adapter_catalog::PLUGIN_ID,
                workflow::PLUGIN_ID,
                marketing::PLUGIN_ID
            ]
        );
        assert!(ctx
            .require::<extension_host::ExtensionHost>(extension_host::SERVICE_KEY)
            .is_ok());
        assert!(ctx
            .require::<adapter_catalog::AdapterCatalog>(adapter_catalog::SERVICE_KEY)
            .is_ok());
        assert!(ctx
            .require::<workflow::WorkflowService>(workflow::SERVICE_KEY)
            .is_ok());
    }

    #[test]
    fn process_kernel_is_initialized_lazily_and_stable() {
        let first = kernel();
        let second = kernel();
        assert!(std::ptr::eq(first, second));
        assert!(extension_host_service().is_some());
        assert!(adapter_catalog_service().is_some());
        assert!(workflow_service().is_some());
        assert!(marketing_service().is_some());
    }

    #[test]
    fn disk_manifest_with_unknown_entry_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("plugin.manifest.json"),
            r#"{"id":"third-party.unknown","name":"Unknown","version":"0.1.0","entry":"builtin:does-not-exist"}"#,
        )
        .unwrap();

        let manifests = collect_manifests(&registry(), &[dir.path().to_path_buf()]);

        assert_eq!(manifests.len(), builtin_manifests().len());
        assert!(!manifests.iter().any(|m| m.id == "third-party.unknown"));
    }

    #[test]
    fn disk_manifest_loads_when_entry_is_registered() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("plugin.manifest.json"),
            r#"{"id":"third-party.echo","name":"Echo","version":"0.1.0","entry":"test:echo","provides":["svc.echo"]}"#,
        )
        .unwrap();
        let mut registry = registry();
        registry.register("test:echo", |ctx| {
            ctx.provide("svc.echo", 7_u8).map_err(String::from)?;
            Ok(())
        });

        let manifests = collect_manifests(&registry, &[dir.path().to_path_buf()]);
        let ctx = Context::new();
        let report = start_plugins(&ctx, &registry, manifests).unwrap();

        assert!(report.loaded.contains(&"third-party.echo"));
        assert_eq!(*ctx.require::<u8>("svc.echo").unwrap(), 7);
    }
}
