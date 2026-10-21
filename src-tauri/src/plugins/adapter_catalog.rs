//! 内置插件：CLI 适配器目录。
//!
//! 把「有哪些 CLI 适配器」从散落的调用点收口成一个内核服务，作为 P4 的第一个
//! 真实业务迁移样板：对外行为不变，只是取数路径变成插件。

use std::sync::Arc;

use crate::agents::adapters::manifest::{catalog, resolve_adapter, AdapterCatalogItem, SpawnSpec};
use crate::kernel::{Context, PluginManifest, PluginRegistry};

pub const ENTRY: &str = "builtin:adapter-catalog";
pub const SERVICE_KEY: &str = "svc.adapters";
pub const PLUGIN_ID: &str = "builtin.adapter-catalog";

/// 适配器目录服务。
#[derive(Debug, Clone, Copy, Default)]
pub struct AdapterCatalog;

impl AdapterCatalog {
    /// 当前可用适配器（内置 + 已加载的 `*.adapter.json`）。
    pub fn list(&self) -> Vec<AdapterCatalogItem> {
        catalog()
    }

    /// 解析一个适配器名，返回启动规格。
    pub fn resolve(&self, name: &str) -> Result<SpawnSpec, String> {
        resolve_adapter(name)
    }
}

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID.into(),
        name: "Adapter Catalog".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        entry: ENTRY.into(),
        provides: vec![SERVICE_KEY.into()],
        inject: vec![],
        permissions: vec![],
    }
}

pub fn register(registry: &mut PluginRegistry) {
    registry.register(ENTRY, |ctx: &Context| {
        ctx.provide(SERVICE_KEY, AdapterCatalog)
            .map_err(String::from)?;
        Ok(())
    });
}

pub fn from_context(ctx: &Context) -> Option<Arc<AdapterCatalog>> {
    ctx.require(SERVICE_KEY).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::{parse_manifest, start_plugins, PluginRegistry};

    #[test]
    fn manifest_matches_the_contract() {
        let raw = serde_json::to_string(&manifest()).unwrap();
        let parsed = parse_manifest(&raw).unwrap();
        assert_eq!(parsed.id, PLUGIN_ID);
        assert_eq!(parsed.entry, ENTRY);
        assert_eq!(parsed.provides, vec![SERVICE_KEY.to_string()]);
    }

    #[test]
    fn plugin_installs_the_catalog_service() {
        let mut registry = PluginRegistry::new();
        register(&mut registry);
        let ctx = Context::new();
        start_plugins(&ctx, &registry, vec![manifest()]).unwrap();

        let service = from_context(&ctx).unwrap();
        assert_eq!(service.list().len(), catalog().len());
        assert!(!service.list().is_empty());
    }

    #[test]
    fn context_without_plugin_has_no_service() {
        let ctx = Context::new();
        assert!(from_context(&ctx).is_none());
    }

    #[test]
    fn resolve_matches_the_free_function() {
        let mut registry = PluginRegistry::new();
        register(&mut registry);
        let ctx = Context::new();
        start_plugins(&ctx, &registry, vec![manifest()]).unwrap();
        let service = from_context(&ctx).unwrap();

        let via_service = format!("{:?}", service.resolve("mock").unwrap());
        let via_function = format!(
            "{:?}",
            crate::agents::adapters::manifest::resolve_adapter("mock").unwrap()
        );
        assert_eq!(via_service, via_function);

        assert!(service.resolve("no-such-adapter").is_err());
    }
}
