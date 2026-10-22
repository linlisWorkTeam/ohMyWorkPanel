//! 内置插件：Self-Marketing（P4）。
//!
//! 已收口：campaign 列表 / 详情 / 创建 / 修订 / 批准 / 导出。
//! 生成与修订需要调度器，由调用方传入 `SchedulerState`（插件不持久化它）。

use std::path::Path;
use std::sync::Arc;

use crate::db::{open_db, AppResult};
use crate::kernel::{Context, PluginManifest, PluginRegistry};
use crate::marketing::{
    approve_campaign, create_campaign, export_campaign, get_campaign, list_campaigns,
    revise_campaign, ApproveCampaignInput, CampaignExport, ContentCampaign, CreateCampaignInput,
    ReviseCampaignInput,
};
use crate::scheduler::SchedulerState;

pub const ENTRY: &str = "builtin:marketing";
pub const SERVICE_KEY: &str = "svc.marketing";
pub const PLUGIN_ID: &str = "builtin.marketing";

/// Self-Marketing 服务。
#[derive(Debug, Clone, Copy, Default)]
pub struct MarketingService;

impl MarketingService {
    pub fn list(&self, db_path: &Path, group_id: &str) -> AppResult<Vec<ContentCampaign>> {
        let conn = open_db(db_path)?;
        list_campaigns(&conn, group_id)
    }

    pub fn get(&self, db_path: &Path, campaign_id: &str) -> AppResult<ContentCampaign> {
        let conn = open_db(db_path)?;
        get_campaign(&conn, campaign_id)
    }

    /// 批准 campaign。
    pub fn approve(
        &self,
        db_path: &Path,
        campaign_id: &str,
        input: ApproveCampaignInput,
    ) -> AppResult<ContentCampaign> {
        approve_campaign(db_path, campaign_id, input)
    }

    /// 导出为 Markdown。
    pub fn export(&self, db_path: &Path, campaign_id: &str) -> AppResult<CampaignExport> {
        let conn = open_db(db_path)?;
        export_campaign(&conn, campaign_id)
    }

    /// 创建 campaign（会派发 Planner/Writer Agent）。
    pub fn create(
        &self,
        db_path: &Path,
        sched: SchedulerState,
        input: CreateCampaignInput,
    ) -> AppResult<ContentCampaign> {
        create_campaign(db_path, sched, input)
    }

    /// 要求修改并重新生成草稿。
    pub fn revise(
        &self,
        db_path: &Path,
        sched: SchedulerState,
        campaign_id: &str,
        input: ReviseCampaignInput,
    ) -> AppResult<ContentCampaign> {
        revise_campaign(db_path, sched, campaign_id, input)
    }
}

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID.into(),
        name: "Marketing".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        entry: ENTRY.into(),
        provides: vec![SERVICE_KEY.into()],
        inject: vec![],
        permissions: vec![],
    }
}

pub fn register(registry: &mut PluginRegistry) {
    registry.register(ENTRY, |ctx: &Context| {
        ctx.provide(SERVICE_KEY, MarketingService)
            .map_err(String::from)?;
        Ok(())
    });
}

pub fn from_context(ctx: &Context) -> Option<Arc<MarketingService>> {
    ctx.require(SERVICE_KEY).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::event_sender::EventSender;
    use crate::kernel::{parse_manifest, start_plugins, PluginRegistry};
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Mutex};
    use tempfile::NamedTempFile;

    fn db() -> NamedTempFile {
        let file = NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        file
    }

    fn sched(path: &Path) -> SchedulerState {
        let (tx, _rx) = tokio::sync::broadcast::channel::<String>(8);
        SchedulerState {
            db_path: path.to_path_buf(),
            event_sender: EventSender::Web(tx),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
            scheduling_groups: Arc::new(Mutex::new(HashSet::new())),
            live_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
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
    fn service_matches_the_free_function_on_empty_db() {
        let file = db();
        let mut registry = PluginRegistry::new();
        register(&mut registry);
        let ctx = Context::new();
        start_plugins(&ctx, &registry, vec![manifest()]).unwrap();
        let service = from_context(&ctx).unwrap();

        let via_service = service.list(file.path(), "g").unwrap();
        let via_function = {
            let conn = open_db(file.path()).unwrap();
            list_campaigns(&conn, "g").unwrap()
        };
        assert_eq!(via_service.len(), via_function.len());
        assert!(service.get(file.path(), "nope").is_err());
    }

    #[test]
    fn write_paths_report_missing_campaign() {
        let file = db();
        let mut registry = PluginRegistry::new();
        register(&mut registry);
        let ctx = Context::new();
        start_plugins(&ctx, &registry, vec![manifest()]).unwrap();
        let service = from_context(&ctx).unwrap();

        let approve_error = service
            .approve(
                file.path(),
                "nope",
                ApproveCampaignInput {
                    actor_member_id: "m1".into(),
                },
            )
            .err()
            .unwrap();
        assert!(!approve_error.is_empty());
        assert!(service.export(file.path(), "nope").is_err());
    }

    #[test]
    fn create_and_revise_are_wired() {
        let file = db();
        let mut registry = PluginRegistry::new();
        register(&mut registry);
        let ctx = Context::new();
        start_plugins(&ctx, &registry, vec![manifest()]).unwrap();
        let service = from_context(&ctx).unwrap();

        let create_error = service
            .create(
                file.path(),
                sched(file.path()),
                CreateCampaignInput {
                    group_id: "no-such-group".into(),
                    requested_by: "m1".into(),
                    planner_agent_id: None,
                    writer_agent_id: None,
                    source_mode: None,
                    base_ref: None,
                },
            )
            .err()
            .unwrap();
        assert!(!create_error.is_empty());

        let revise_error = service
            .revise(
                file.path(),
                sched(file.path()),
                "nope",
                ReviseCampaignInput {
                    actor_member_id: "m1".into(),
                    feedback: "再改改".into(),
                },
            )
            .err()
            .unwrap();
        assert!(!revise_error.is_empty());
    }

    #[test]
    fn context_without_plugin_has_no_service() {
        let ctx = Context::new();
        assert!(from_context(&ctx).is_none());
    }
}
