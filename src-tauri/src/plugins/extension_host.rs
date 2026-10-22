//! 内置插件：把现有能力包成 kernel 插件。
//!
//! 这一层的意义是验证插件契约：插件的对外行为必须与原自由函数一致，
//! 否则就说明内核抽象没有对上真实需求。

use std::path::PathBuf;
use std::sync::Arc;

use crate::db::AppResult;
use crate::extensions::{discover_extensions_from, find_extension_in, DiscoveredExtension};
use crate::kernel::{Context, PluginManifest, PluginRegistry};

/// 注册表里的实现入口名。
pub const ENTRY: &str = "builtin:extension-host";
/// 上下文里的服务 key。
pub const SERVICE_KEY: &str = "svc.extensions";
/// 插件 id。
pub const PLUGIN_ID: &str = "builtin.extension-host";

/// 扩展宿主插件提供的服务。
///
/// 根目录在安装时快照，避免每次发现都重新读环境变量；热重载留给后续切片。
#[derive(Debug, Clone)]
pub struct ExtensionHost {
    roots: Vec<PathBuf>,
}

impl ExtensionHost {
    pub fn new() -> Self {
        Self {
            roots: crate::extensions::extension_roots(),
        }
    }

    pub fn with_roots(roots: Vec<PathBuf>) -> Self {
        Self { roots }
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    pub fn discover(&self) -> Vec<DiscoveredExtension> {
        discover_extensions_from(&self.roots)
    }

    pub fn find(&self, ext_id: &str) -> AppResult<DiscoveredExtension> {
        find_extension_in(&self.roots, ext_id)
    }
}

impl Default for ExtensionHost {
    fn default() -> Self {
        Self::new()
    }
}

/// 内置清单：编译期构造，不依赖磁盘上的 `plugin.manifest.json`。
pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID.into(),
        name: "Extension Host".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        entry: ENTRY.into(),
        provides: vec![SERVICE_KEY.into()],
        inject: vec![],
        permissions: vec!["fs.read".into()],
    }
}

/// 把安装函数登记到注册表。
pub fn register(registry: &mut PluginRegistry) {
    registry.register(ENTRY, |ctx: &Context| {
        ctx.provide(SERVICE_KEY, ExtensionHost::new())
            .map_err(String::from)?;
        Ok(())
    });
}

/// 供启动流程复用：装载后取回服务。
pub fn from_context(ctx: &Context) -> Option<Arc<ExtensionHost>> {
    ctx.require(SERVICE_KEY).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::{parse_manifest, start_plugins, PluginRegistry};

    fn load() -> (Context, Arc<ExtensionHost>) {
        let mut registry = PluginRegistry::new();
        register(&mut registry);
        let ctx = Context::new();
        start_plugins(&ctx, &registry, vec![manifest()]).unwrap();
        let host = from_context(&ctx).unwrap();
        (ctx, host)
    }

    #[test]
    fn manifest_matches_the_manifest_contract() {
        let raw = serde_json::to_string(&manifest()).unwrap();
        let parsed = parse_manifest(&raw).unwrap();
        assert_eq!(parsed.id, PLUGIN_ID);
        assert_eq!(parsed.entry, ENTRY);
        assert_eq!(parsed.provides, vec![SERVICE_KEY.to_string()]);
    }

    #[test]
    fn plugin_installs_extension_host_service() {
        let (_ctx, host) = load();
        assert_eq!(
            host.roots(),
            crate::extensions::extension_roots().as_slice()
        );
    }

    #[test]
    fn service_discovery_matches_the_free_function() {
        let (_ctx, host) = load();
        let via_service: Vec<String> = host
            .discover()
            .into_iter()
            .map(|found| found.manifest.id)
            .collect();
        let via_function: Vec<String> = crate::extensions::discover_extensions()
            .into_iter()
            .map(|found| found.manifest.id)
            .collect();
        assert_eq!(via_service, via_function);
    }

    #[test]
    fn unknown_extension_is_reported() {
        let (_ctx, host) = load();
        let error = host.find("no-such-ext").err().unwrap();
        assert!(error.contains("no-such-ext"));
    }

    #[test]
    fn context_without_plugins_has_no_service() {
        let ctx = Context::new();
        assert!(from_context(&ctx).is_none());
    }
}
