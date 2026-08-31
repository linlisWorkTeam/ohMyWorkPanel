use super::models::{
    ChannelDraft, ContentBrief, ContentCampaign, RepositorySnapshot, ValidationFinding,
};
use crate::db::AppResult;
use rusqlite::{params, Connection, OptionalExtension, Row};

const SELECT: &str = "SELECT id,group_id,requested_by,planner_agent_id,writer_agent_id,status,source_mode,base_ref,head_ref,snapshot_json,brief_json,drafts_json,validation_json,planner_run_id,writer_run_id,revision,feedback,feedback_by,error_message,approved_by,created_at,updated_at FROM content_campaigns";

fn decode<T: serde::de::DeserializeOwned>(raw: &str, field: &str) -> rusqlite::Result<T> {
    serde_json::from_str(raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            format!("content_campaigns.{field}: {e}").into(),
        )
    })
}

fn campaign_from_row(row: &Row<'_>) -> rusqlite::Result<ContentCampaign> {
    let snapshot_json: String = row.get(9)?;
    let brief_json: Option<String> = row.get(10)?;
    let drafts_json: String = row.get(11)?;
    let validation_json: String = row.get(12)?;
    Ok(ContentCampaign {
        id: row.get(0)?,
        group_id: row.get(1)?,
        requested_by: row.get(2)?,
        planner_agent_id: row.get(3)?,
        writer_agent_id: row.get(4)?,
        status: row.get(5)?,
        source_mode: row.get(6)?,
        base_ref: row.get(7)?,
        head_ref: row.get(8)?,
        snapshot: decode::<RepositorySnapshot>(&snapshot_json, "snapshot_json")?,
        brief: brief_json
            .as_deref()
            .map(|raw| decode::<ContentBrief>(raw, "brief_json"))
            .transpose()?,
        drafts: decode::<Vec<ChannelDraft>>(&drafts_json, "drafts_json")?,
        validation: decode::<Vec<ValidationFinding>>(&validation_json, "validation_json")?,
        planner_run_id: row.get(13)?,
        writer_run_id: row.get(14)?,
        revision: row.get(15)?,
        feedback: row.get(16)?,
        feedback_by: row.get(17)?,
        error_message: row.get(18)?,
        approved_by: row.get(19)?,
        created_at: row.get(20)?,
        updated_at: row.get(21)?,
    })
}

pub fn insert_campaign(conn: &Connection, campaign: &ContentCampaign) -> AppResult<()> {
    let snapshot = serde_json::to_string(&campaign.snapshot).map_err(|e| e.to_string())?;
    let brief = campaign
        .brief
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| e.to_string())?;
    let drafts = serde_json::to_string(&campaign.drafts).map_err(|e| e.to_string())?;
    let validation = serde_json::to_string(&campaign.validation).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO content_campaigns(id,group_id,requested_by,planner_agent_id,writer_agent_id,status,source_mode,base_ref,head_ref,snapshot_json,brief_json,drafts_json,validation_json,planner_run_id,writer_run_id,revision,feedback,feedback_by,error_message,approved_by,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)",
        params![
            campaign.id,
            campaign.group_id,
            campaign.requested_by,
            campaign.planner_agent_id,
            campaign.writer_agent_id,
            campaign.status,
            campaign.source_mode,
            campaign.base_ref,
            campaign.head_ref,
            snapshot,
            brief,
            drafts,
            validation,
            campaign.planner_run_id,
            campaign.writer_run_id,
            campaign.revision,
            campaign.feedback,
            campaign.feedback_by,
            campaign.error_message,
            campaign.approved_by,
            campaign.created_at,
            campaign.updated_at,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_campaign(conn: &Connection, campaign_id: &str) -> AppResult<ContentCampaign> {
    conn.query_row(
        &format!("{SELECT} WHERE id=?1"),
        params![campaign_id],
        campaign_from_row,
    )
    .map_err(|e| format!("宣传 Campaign 不存在或数据损坏：{e}"))
}

pub fn find_by_run(
    conn: &Connection,
    run_id: &str,
) -> AppResult<Option<(ContentCampaign, String)>> {
    let planner = conn
        .query_row(
            &format!("{SELECT} WHERE planner_run_id=?1 ORDER BY updated_at DESC LIMIT 1"),
            params![run_id],
            campaign_from_row,
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(campaign) = planner {
        return Ok(Some((campaign, "planning".into())));
    }
    conn.query_row(
        &format!("{SELECT} WHERE writer_run_id=?1 ORDER BY updated_at DESC LIMIT 1"),
        params![run_id],
        campaign_from_row,
    )
    .optional()
    .map(|value| value.map(|campaign| (campaign, "writing".into())))
    .map_err(|e| e.to_string())
}

pub fn list_campaigns(conn: &Connection, group_id: &str) -> AppResult<Vec<ContentCampaign>> {
    let mut stmt = conn
        .prepare(&format!(
            "{SELECT} WHERE group_id=?1 ORDER BY updated_at DESC LIMIT 50"
        ))
        .map_err(|e| e.to_string())?;
    let campaigns = stmt
        .query_map(params![group_id], campaign_from_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(campaigns)
}

pub fn save_campaign(conn: &Connection, campaign: &ContentCampaign) -> AppResult<()> {
    let snapshot = serde_json::to_string(&campaign.snapshot).map_err(|e| e.to_string())?;
    let brief = campaign
        .brief
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| e.to_string())?;
    let drafts = serde_json::to_string(&campaign.drafts).map_err(|e| e.to_string())?;
    let validation = serde_json::to_string(&campaign.validation).map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE content_campaigns SET status=?1,source_mode=?2,base_ref=?3,head_ref=?4,snapshot_json=?5,brief_json=?6,drafts_json=?7,validation_json=?8,planner_run_id=?9,writer_run_id=?10,revision=?11,feedback=?12,feedback_by=?13,error_message=?14,approved_by=?15,updated_at=?16 WHERE id=?17",
        params![
            campaign.status,
            campaign.source_mode,
            campaign.base_ref,
            campaign.head_ref,
            snapshot,
            brief,
            drafts,
            validation,
            campaign.planner_run_id,
            campaign.writer_run_id,
            campaign.revision,
            campaign.feedback,
            campaign.feedback_by,
            campaign.error_message,
            campaign.approved_by,
            campaign.updated_at,
            campaign.id,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
