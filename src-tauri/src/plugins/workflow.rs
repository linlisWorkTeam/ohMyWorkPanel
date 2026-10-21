//! 内置插件：版本 / Wave 工作流（P4 深水区）。
//!
//! 已收口：版本板读数 + 版本创建。其余写操作仍走既有 use case。

use std::path::Path;
use std::sync::Arc;

use crate::db::{open_db, AppResult};
use crate::kernel::{Context, PluginManifest, PluginRegistry};
use crate::models::Group;
use crate::workflow::{
    board_for_group, create_version, CreateVersionInput, ProjectVersion, VersionBoard,
};

pub const ENTRY: &str = "builtin:workflow";
pub const SERVICE_KEY: &str = "svc.workflow";
pub const PLUGIN_ID: &str = "builtin.workflow";

/// 工作流服务。
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkflowService;

impl WorkflowService {
    /// 群版本板（Git 快照 + 版本 + Wave + asking）。
    pub fn board(&self, db_path: &Path, group: &Group) -> AppResult<VersionBoard> {
        board_for_group(db_path, group)
    }

    /// 新建版本（有副作用的写用例）。
    pub fn create_version(
        &self,
        db_path: &Path,
        group: &Group,
        input: &CreateVersionInput,
    ) -> AppResult<ProjectVersion> {
        let conn = open_db(db_path)?;
        create_version(&conn, group, input)
    }
}

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID.into(),
        name: "Workflow".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        entry: ENTRY.into(),
        provides: vec![SERVICE_KEY.into()],
        inject: vec![],
        permissions: vec![],
    }
}

pub fn register(registry: &mut PluginRegistry) {
    registry.register(ENTRY, |ctx: &Context| {
        ctx.provide(SERVICE_KEY, WorkflowService)
            .map_err(String::from)?;
        Ok(())
    });
}

pub fn from_context(ctx: &Context) -> Option<Arc<WorkflowService>> {
    ctx.require(SERVICE_KEY).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{init_db, open_db};
    use crate::kernel::{parse_manifest, start_plugins, PluginRegistry};
    use tempfile::NamedTempFile;

    fn group_db() -> (NamedTempFile, Group) {
        let file = NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        conn.execute(
            "INSERT INTO groups (id, name, workspace_path, owner_member_id, created_at)
             VALUES ('g', '群', '.', 'm-owner', 1)",
            [],
        )
        .unwrap();
        let group = crate::db::get_group(&conn, "g").unwrap();
        (file, group)
    }

    #[test]
    fn manifest_matches_the_contract() {
        let raw = serde_json::to_string(&manifest()).unwrap();
        let parsed = parse_manifest(&raw).unwrap();
        assert_eq!(parsed.id, PLUGIN_ID);
        assert_eq!(parsed.entry, ENTRY);
        assert_eq!(parsed.provides, vec![SERVICE_KEY.to_string()]);
    }

    #[test]
    fn service_board_matches_the_free_function() {
        let (file, group) = group_db();
        let mut registry = PluginRegistry::new();
        register(&mut registry);
        let ctx = Context::new();
        start_plugins(&ctx, &registry, vec![manifest()]).unwrap();
        let service = from_context(&ctx).unwrap();

        let via_service = service.board(file.path(), &group).unwrap();
        let via_function = board_for_group(file.path(), &group).unwrap();

        assert_eq!(via_service.versions.len(), via_function.versions.len());
        assert_eq!(via_service.waves.len(), via_function.waves.len());
        assert_eq!(
            via_service.asking_version_id,
            via_function.asking_version_id
        );
    }

    #[test]
    fn service_creates_a_version_like_the_free_function() {
        let (file, mut group) = group_db();
        group.admin_member_id = Some("m-agent".into());
        let mut registry = PluginRegistry::new();
        register(&mut registry);
        let ctx = Context::new();
        start_plugins(&ctx, &registry, vec![manifest()]).unwrap();
        let service = from_context(&ctx).unwrap();
        let input = CreateVersionInput {
            group_id: "g".into(),
            name: Some("v-next".into()),
            what: Some("what".into()),
            who: Some("who".into()),
            how: Some("how".into()),
            one_liner: Some("one".into()),
            requester_member_id: None,
            mode: Some("create".into()),
        };

        let via_service = service.create_version(file.path(), &group, &input).unwrap();
        assert_eq!(via_service.status, "planning");
        assert_eq!(via_service.kind, "draft");
        assert_eq!(via_service.name, "v-next");
        assert_eq!(via_service.group_id, "g");

        let board = service.board(file.path(), &group).unwrap();
        assert!(board
            .versions
            .iter()
            .any(|version| version.id == via_service.id));
    }

    #[test]
    fn context_without_plugin_has_no_service() {
        let ctx = Context::new();
        assert!(from_context(&ctx).is_none());
    }
}
