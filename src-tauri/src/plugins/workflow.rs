//! 内置插件：版本 / Wave 工作流（P4 深水区）。
//!
//! 已收口：版本板读数 + 版本创建。其余写操作仍走既有 use case。

use std::path::Path;
use std::sync::Arc;

use crate::db::{open_db, AppResult};
use crate::kernel::{Context, PluginManifest, PluginRegistry};
use crate::models::Group;
use crate::workflow::{
    board_for_group, cancel_ask, create_version, mark_released, pause_wave, play_wave, start_ask,
    update_roadmap, CreateVersionInput, ProjectVersion, UpdateRoadmapInput, VersionBoard, Wave,
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

    /// 更新 Roadmap（What / Who / How / 一句话 / 名称）。
    pub fn update_roadmap(
        &self,
        db_path: &Path,
        version_id: &str,
        input: &UpdateRoadmapInput,
    ) -> AppResult<ProjectVersion> {
        let conn = open_db(db_path)?;
        update_roadmap(&conn, version_id, input)
    }

    /// 播放 Wave（状态机写操作）。
    pub fn play_wave(&self, db_path: &Path, wave_id: &str) -> AppResult<Wave> {
        let conn = open_db(db_path)?;
        play_wave(&conn, wave_id)
    }

    /// 暂停 Wave。
    pub fn pause_wave(&self, db_path: &Path, wave_id: &str) -> AppResult<Wave> {
        let conn = open_db(db_path)?;
        pause_wave(&conn, wave_id)
    }

    /// 进入 Ask 模式。
    pub fn start_ask(
        &self,
        db_path: &Path,
        group: &Group,
        version_id: &str,
    ) -> AppResult<ProjectVersion> {
        let conn = open_db(db_path)?;
        start_ask(&conn, group, version_id)
    }

    /// 取消 Ask。
    pub fn cancel_ask(&self, db_path: &Path, version_id: &str) -> AppResult<ProjectVersion> {
        let conn = open_db(db_path)?;
        cancel_ask(&conn, version_id)
    }

    /// 标记发布（可携带 Git tag）。
    pub fn mark_released(
        &self,
        db_path: &Path,
        version_id: &str,
        git_tag: Option<String>,
    ) -> AppResult<ProjectVersion> {
        let conn = open_db(db_path)?;
        mark_released(&conn, version_id, git_tag)
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
    fn service_updates_roadmap_like_the_free_function() {
        let (file, mut group) = group_db();
        group.admin_member_id = Some("m-agent".into());
        let mut registry = PluginRegistry::new();
        register(&mut registry);
        let ctx = Context::new();
        start_plugins(&ctx, &registry, vec![manifest()]).unwrap();
        let service = from_context(&ctx).unwrap();
        let created = service
            .create_version(
                file.path(),
                &group,
                &CreateVersionInput {
                    group_id: "g".into(),
                    name: Some("v-roadmap".into()),
                    what: None,
                    who: None,
                    how: None,
                    one_liner: None,
                    requester_member_id: None,
                    mode: Some("create".into()),
                },
            )
            .unwrap();

        let updated = service
            .update_roadmap(
                file.path(),
                &created.id,
                &UpdateRoadmapInput {
                    what: Some("做版本页".into()),
                    who: Some("Codex".into()),
                    how: Some("先契约后实现".into()),
                    one_liner: Some("一句话".into()),
                    name: None,
                    requester_member_id: None,
                },
            )
            .unwrap();

        assert_eq!(updated.what, "做版本页");
        assert_eq!(updated.who, "Codex");
        assert_eq!(updated.how, "先契约后实现");
        let board = service.board(file.path(), &group).unwrap();
        let reloaded = board
            .versions
            .iter()
            .find(|version| version.id == created.id)
            .unwrap();
        assert_eq!(reloaded.what, "做版本页");
    }

    #[test]
    fn service_plays_and_pauses_a_wave() {
        let (file, mut group) = group_db();
        group.admin_member_id = Some("m-agent".into());
        let mut registry = PluginRegistry::new();
        register(&mut registry);
        let ctx = Context::new();
        start_plugins(&ctx, &registry, vec![manifest()]).unwrap();
        let service = from_context(&ctx).unwrap();
        let version = service
            .create_version(
                file.path(),
                &group,
                &CreateVersionInput {
                    group_id: "g".into(),
                    name: Some("v-wave".into()),
                    what: None,
                    who: None,
                    how: None,
                    one_liner: None,
                    requester_member_id: None,
                    mode: Some("create".into()),
                },
            )
            .unwrap();
        {
            let conn = open_db(file.path()).unwrap();
            conn.execute(
                "INSERT INTO waves (id, version_id, group_id, idx, title, status, phase, phase_cursor, play_state, summary, created_at, updated_at)
                 VALUES ('w1', ?1, 'g', 0, 'Wave1', 'idle', 'assign', 'assign', 'idle', '', 1, 1)",
                [&version.id],
            )
            .unwrap();
        }

        let playing = service.play_wave(file.path(), "w1").unwrap();
        assert_eq!(playing.status, "running");
        assert_eq!(playing.play_state, "playing");

        let paused = service.pause_wave(file.path(), "w1").unwrap();
        assert_eq!(paused.status, "paused");
        assert_eq!(paused.play_state, "paused");
    }

    #[test]
    fn service_starts_and_cancels_ask() {
        let (file, mut group) = group_db();
        group.admin_member_id = Some("m-agent".into());
        let mut registry = PluginRegistry::new();
        register(&mut registry);
        let ctx = Context::new();
        start_plugins(&ctx, &registry, vec![manifest()]).unwrap();
        let service = from_context(&ctx).unwrap();
        let version = service
            .create_version(
                file.path(),
                &group,
                &CreateVersionInput {
                    group_id: "g".into(),
                    name: Some("v-ask".into()),
                    what: Some("对齐需求".into()),
                    who: Some("Codex".into()),
                    how: Some("先问再做".into()),
                    one_liner: None,
                    requester_member_id: None,
                    mode: Some("create".into()),
                },
            )
            .unwrap();

        let asking = service.start_ask(file.path(), &group, &version.id).unwrap();
        assert_eq!(asking.status, "asking");
        assert_eq!(
            service
                .board(file.path(), &group)
                .unwrap()
                .asking_version_id,
            Some(version.id.clone())
        );

        let cancelled = service.cancel_ask(file.path(), &version.id).unwrap();
        assert_eq!(cancelled.status, "planning");
        assert!(service
            .board(file.path(), &group)
            .unwrap()
            .asking_version_id
            .is_none());
    }

    #[test]
    fn service_marks_a_version_released() {
        let (file, mut group) = group_db();
        group.admin_member_id = Some("m-agent".into());
        let mut registry = PluginRegistry::new();
        register(&mut registry);
        let ctx = Context::new();
        start_plugins(&ctx, &registry, vec![manifest()]).unwrap();
        let service = from_context(&ctx).unwrap();
        let version = service
            .create_version(
                file.path(),
                &group,
                &CreateVersionInput {
                    group_id: "g".into(),
                    name: Some("v-release".into()),
                    what: None,
                    who: None,
                    how: None,
                    one_liner: None,
                    requester_member_id: None,
                    mode: Some("create".into()),
                },
            )
            .unwrap();

        let released = service
            .mark_released(file.path(), &version.id, Some("v9.9.9".into()))
            .unwrap();

        assert_eq!(released.status, "released");
        assert_eq!(released.git_tag.as_deref(), Some("v9.9.9"));
        assert_eq!(released.kind, "tag");
        assert!(released.released_at.is_some());
    }

    #[test]
    fn context_without_plugin_has_no_service() {
        let ctx = Context::new();
        assert!(from_context(&ctx).is_none());
    }
}
