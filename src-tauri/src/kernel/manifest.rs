//! 插件清单 v2：文件格式、目录发现与注册表映射。
//!
//! 清单描述「要装什么、依赖谁、由谁提供实现」，实现本体仍由进程内注册表提供
//! （v0 不做动态库加载）。清单里的字符串在启动期泄漏为 `&'static str`，
//! 因为插件规格与进程同生命周期，这样做可以避免给内核 API 增加所有权复杂度。

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;

use super::context::Context;
use super::plugin::{load_plugins, InstallFn, LoadReport, PluginSpec};

/// 清单文件名；每个插件目录放一个。
pub const PLUGIN_MANIFEST_FILE: &str = "plugin.manifest.json";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub version: String,
    /// 实现入口名，对应注册表里的 key（如 `builtin:echo`）。
    pub entry: String,
    #[serde(default)]
    pub provides: Vec<String>,
    #[serde(default)]
    pub inject: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    Parse(String),
    EmptyField(&'static str),
    DuplicateKey { field: &'static str, key: String },
    ConflictingKey { key: String },
    UnknownEntry(String),
    Load(String),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(message) => write!(f, "清单解析失败：{message}"),
            Self::EmptyField(field) => write!(f, "清单字段为空：{field}"),
            Self::DuplicateKey { field, key } => write!(f, "清单 {field} 重复：{key}"),
            Self::ConflictingKey { key } => {
                write!(f, "服务 key 同时出现在 provides 与 inject：{key}")
            }
            Self::UnknownEntry(entry) => write!(f, "未注册的插件入口：{entry}"),
            Self::Load(message) => write!(f, "插件装载失败：{message}"),
        }
    }
}

impl std::error::Error for ManifestError {}

impl From<ManifestError> for String {
    fn from(error: ManifestError) -> Self {
        error.to_string()
    }
}

fn require(value: &str, field: &'static str) -> Result<(), ManifestError> {
    if value.trim().is_empty() {
        Err(ManifestError::EmptyField(field))
    } else {
        Ok(())
    }
}

fn validate(manifest: &PluginManifest) -> Result<(), ManifestError> {
    require(&manifest.id, "id")?;
    require(&manifest.version, "version")?;
    require(&manifest.entry, "entry")?;

    let mut provides = std::collections::HashSet::new();
    for key in &manifest.provides {
        if !provides.insert(key) {
            return Err(ManifestError::DuplicateKey {
                field: "provides",
                key: key.clone(),
            });
        }
    }
    let mut inject = std::collections::HashSet::new();
    for key in &manifest.inject {
        if !inject.insert(key) {
            return Err(ManifestError::DuplicateKey {
                field: "inject",
                key: key.clone(),
            });
        }
    }
    for key in &manifest.inject {
        if provides.contains(key) {
            return Err(ManifestError::ConflictingKey { key: key.clone() });
        }
    }
    Ok(())
}

pub fn parse_manifest(raw: &str) -> Result<PluginManifest, ManifestError> {
    let manifest: PluginManifest =
        serde_json::from_str(raw).map_err(|error| ManifestError::Parse(error.to_string()))?;
    validate(&manifest)?;
    Ok(manifest)
}

/// 一个目录的发现结果；`root` 便于报错定位。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovery {
    pub root: PathBuf,
    pub result: Result<PluginManifest, ManifestError>,
}

/// 扫描根目录，返回所有包含 `plugin.manifest.json` 的插件。
pub fn discover_manifests(roots: &[PathBuf]) -> Vec<Discovery> {
    let mut found = Vec::new();
    for root in roots {
        let path: &Path = &root.join(PLUGIN_MANIFEST_FILE);
        if !path.is_file() {
            continue;
        }
        let result = std::fs::read_to_string(path)
            .map_err(|error| {
                ManifestError::Parse(format!("读取清单失败（{}）：{error}", path.display()))
            })
            .and_then(|raw| parse_manifest(&raw));
        found.push(Discovery {
            root: root.clone(),
            result,
        });
    }
    found
}

/// 进程内实现注册表：清单的 `entry` 映射到这里。
#[derive(Default)]
pub struct PluginRegistry {
    entries: HashMap<String, InstallFn>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        entry: impl Into<String>,
        install: impl Fn(&Context) -> Result<(), String> + Send + Sync + 'static,
    ) {
        self.entries.insert(entry.into(), Arc::new(install));
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.entries.contains_key(entry)
    }

    fn resolve(&self, entry: &str) -> Option<InstallFn> {
        self.entries.get(entry).cloned()
    }
}

fn leak(value: &str) -> &'static str {
    Box::leak(value.to_string().into_boxed_str())
}

/// 把清单映射成装载器可用的规格；未知 entry 直接拒绝。
pub fn manifests_to_specs(
    registry: &PluginRegistry,
    manifests: Vec<PluginManifest>,
) -> Result<Vec<PluginSpec>, ManifestError> {
    manifests
        .into_iter()
        .map(|manifest| {
            let install = registry
                .resolve(&manifest.entry)
                .ok_or_else(|| ManifestError::UnknownEntry(manifest.entry.clone()))?;
            Ok(PluginSpec {
                id: leak(&manifest.id),
                provides: manifest.provides.iter().map(|key| leak(key)).collect(),
                inject: manifest.inject.iter().map(|key| leak(key)).collect(),
                install,
            })
        })
        .collect()
}

/// 一步到位：清单 → 规格 → 按依赖装载。
pub fn start_plugins(
    ctx: &Context,
    registry: &PluginRegistry,
    manifests: Vec<PluginManifest>,
) -> Result<LoadReport, ManifestError> {
    let specs = manifests_to_specs(registry, manifests)?;
    load_plugins(ctx, specs).map_err(|error| ManifestError::Load(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn valid() -> &'static str {
        r#"{
            "id": "demo.echo",
            "name": "Echo",
            "version": "0.1.0",
            "entry": "builtin:echo",
            "provides": ["svc.echo"]
        }"#
    }

    #[test]
    fn parses_a_valid_manifest() {
        let manifest = parse_manifest(valid()).unwrap();
        assert_eq!(manifest.id, "demo.echo");
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.entry, "builtin:echo");
        assert_eq!(manifest.provides, vec!["svc.echo"]);
        assert!(manifest.inject.is_empty());
        assert!(manifest.permissions.is_empty());
    }

    #[test]
    fn rejects_empty_required_fields() {
        let error = parse_manifest(r#"{"id":" ","version":"1","entry":"e"}"#).unwrap_err();
        assert_eq!(error, ManifestError::EmptyField("id"));
    }

    #[test]
    fn rejects_duplicate_keys() {
        let error = parse_manifest(r#"{"id":"a","version":"1","entry":"e","provides":["x","x"]}"#)
            .unwrap_err();
        assert_eq!(
            error,
            ManifestError::DuplicateKey {
                field: "provides",
                key: "x".into()
            }
        );
    }

    #[test]
    fn rejects_keys_both_provided_and_injected() {
        let error = parse_manifest(
            r#"{"id":"a","version":"1","entry":"e","provides":["x"],"inject":["x"]}"#,
        )
        .unwrap_err();
        assert_eq!(error, ManifestError::ConflictingKey { key: "x".into() });
    }

    #[test]
    fn unknown_entry_is_rejected_before_loading() {
        let registry = PluginRegistry::new();
        let manifest = parse_manifest(valid()).unwrap();
        let error = manifests_to_specs(&registry, vec![manifest]).err().unwrap();
        assert_eq!(error, ManifestError::UnknownEntry("builtin:echo".into()));
    }

    #[test]
    fn discovers_manifests_from_roots() {
        let dir = tempfile::tempdir().unwrap();
        let mut file = std::fs::File::create(dir.path().join(PLUGIN_MANIFEST_FILE)).unwrap();
        file.write_all(valid().as_bytes()).unwrap();
        let empty = tempfile::tempdir().unwrap();

        let found = discover_manifests(&[dir.path().to_path_buf(), empty.path().to_path_buf()]);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].result.as_ref().unwrap().id, "demo.echo");
    }

    #[test]
    fn end_to_end_lists_and_loads_a_plugin() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(PLUGIN_MANIFEST_FILE),
            r#"{"id":"demo.echo","version":"0.1.0","entry":"builtin:echo","provides":["svc.echo"]}"#,
        )
        .unwrap();
        let mut registry = PluginRegistry::new();
        registry.register("builtin:echo", |ctx| {
            ctx.provide("svc.echo", "hi").map_err(String::from)?;
            Ok(())
        });
        let manifests: Vec<PluginManifest> = discover_manifests(&[dir.path().to_path_buf()])
            .into_iter()
            .map(|found| found.result.unwrap())
            .collect();

        let ctx = Context::new();
        let report = start_plugins(&ctx, &registry, manifests).unwrap();

        assert_eq!(report.loaded, vec!["demo.echo"]);
        assert_eq!(*ctx.require::<&'static str>("svc.echo").unwrap(), "hi");
    }
}
