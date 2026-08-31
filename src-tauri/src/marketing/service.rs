use super::context::collect_repository_snapshot;
use super::models::{
    ApproveCampaignInput, CampaignExport, ContentBrief, ContentCampaign, CreateCampaignInput,
    DraftBundle, ReviseCampaignInput, ValidationFinding, REQUIRED_CHANNELS,
};
use super::repository::{find_by_run, get_campaign, insert_campaign, save_campaign};
use super::validator::{validate_brief, validate_drafts};
use crate::db::{create_task_run, get_group, get_members, id, now, open_db, AppResult};
use crate::message_content::{apply_channel_delta, parts_to_plain_text};
use crate::models::{ChatEvent, Member};
use crate::scheduler::{self, SchedulerState};
use rusqlite::{params, Connection};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;

const INTERNAL_PREFIX: &str = "[[MARKETING_INTERNAL:";
const CAMPAIGN_PREFIX: &str = "[[MARKETING_CAMPAIGN:";

fn internal_marker(campaign_id: &str, stage: &str) -> String {
    format!("{INTERNAL_PREFIX}{campaign_id}:{stage}]]")
}

fn campaign_marker(campaign_id: &str) -> String {
    format!("{CAMPAIGN_PREFIX}{campaign_id}]]")
}

fn parse_internal_marker(value: &str) -> Option<(&str, &str)> {
    let trimmed = value.trim();
    let body = trimmed.strip_prefix(INTERNAL_PREFIX)?.strip_suffix("]]")?;
    body.rsplit_once(':')
}

fn require_member(conn: &Connection, group_id: &str, member_id: &str) -> AppResult<()> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM members WHERE id=?1 AND group_id=?2 AND is_active=1",
            params![member_id, group_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if count != 1 {
        return Err("操作人不是本群有效成员。".into());
    }
    Ok(())
}

fn usable_agents(conn: &Connection, group_id: &str) -> AppResult<Vec<Member>> {
    Ok(get_members(conn, group_id)?
        .into_iter()
        .filter(|m| m.is_active && matches!(m.kind.as_str(), "agent" | "chatbot"))
        .collect())
}

fn resolve_agent(
    agents: &[Member],
    requested: Option<&str>,
    preferred_terms: &[&str],
    fallback: Option<&str>,
) -> AppResult<String> {
    if let Some(requested) = requested.map(str::trim).filter(|s| !s.is_empty()) {
        return agents
            .iter()
            .find(|agent| agent.id == requested)
            .map(|agent| agent.id.clone())
            .ok_or_else(|| "指定的 Self-Marketing Agent 不可用。".into());
    }
    if let Some(agent) = agents.iter().find(|agent| {
        preferred_terms.iter().any(|term| {
            agent
                .display_name
                .to_ascii_lowercase()
                .contains(&term.to_ascii_lowercase())
                || agent
                    .role_description
                    .to_ascii_lowercase()
                    .contains(&term.to_ascii_lowercase())
                || agent
                    .tags
                    .to_ascii_lowercase()
                    .contains(&term.to_ascii_lowercase())
        })
    }) {
        return Ok(agent.id.clone());
    }
    if let Some(fallback) = fallback {
        if agents.iter().any(|agent| agent.id == fallback) {
            return Ok(fallback.to_string());
        }
    }
    agents
        .first()
        .map(|agent| agent.id.clone())
        .ok_or_else(|| "群内没有可用于 Self-Marketing 的 Agent。".into())
}

fn insert_internal_run(
    conn: &Connection,
    campaign: &ContentCampaign,
    stage: &str,
    agent_id: &str,
) -> AppResult<String> {
    let message_id = id();
    conn.execute(
        "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES(?1,?2,?3,NULL,?4,'completed',?5)",
        params![
            message_id,
            campaign.group_id,
            campaign.requested_by,
            internal_marker(&campaign.id, stage),
            now(),
        ],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO mentions(message_id,member_id) VALUES(?1,?2)",
        params![message_id, agent_id],
    )
    .map_err(|e| e.to_string())?;
    create_task_run(conn, &campaign.group_id, &message_id, agent_id, None, 0)
}

fn insert_card_message(
    conn: &Connection,
    campaign: &ContentCampaign,
    sender_id: &str,
) -> AppResult<String> {
    let message_id = id();
    conn.execute(
        "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES(?1,?2,?3,NULL,?4,'completed',?5)",
        params![message_id, campaign.group_id, sender_id, campaign_marker(&campaign.id), now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(message_id)
}

fn compact_snapshot(campaign: &ContentCampaign) -> serde_json::Value {
    let evidence = campaign
        .snapshot
        .evidence
        .iter()
        .take(18)
        .map(|item| {
            let excerpt = item.excerpt.chars().take(650).collect::<String>();
            json!({
                "id": item.id,
                "kind": item.kind,
                "source": item.source,
                "excerpt": excerpt,
                "contentHash": item.content_hash,
                "releaseState": item.release_state,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "baseRef": campaign.snapshot.base_ref,
        "headRef": campaign.snapshot.head_ref,
        "sourceMode": campaign.snapshot.source_mode,
        "commits": campaign.snapshot.commits,
        "changedFiles": campaign.snapshot.changed_files,
        "uncommittedFiles": campaign.snapshot.uncommitted_files,
        "evidence": evidence,
        "projectContext": campaign.snapshot.config.project_context.chars().take(1600).collect::<String>(),
        "brandGuide": campaign.snapshot.config.brand_guide.chars().take(1600).collect::<String>(),
        "bannedPhrases": campaign.snapshot.config.banned_phrases,
        "snapshotTruncated": campaign.snapshot.truncated,
    })
}

fn planner_prompt(campaign: &ContentCampaign) -> String {
    let snapshot =
        serde_json::to_string_pretty(&compact_snapshot(campaign)).unwrap_or_else(|_| "{}".into());
    format!(
        r#"【Self-Marketing / Content Planner】
你负责判断最近项目更新是否值得对外传播，并生成唯一事实源 Content Brief。

硬规则：
1. 只能使用下面 snapshot 的事实。每个 update / proofPoint 都必须引用真实 evidenceRefs。
2. 未提交证据只能标记 releaseState=unreleased，不能写成已发布。
3. 不值得宣传时必须选择 no_content；不要为了交差制造卖点。
4. 禁止夸大、行业排名、无证据性能数字和绝对承诺。
5. 只输出一个 JSON 对象，不要 Markdown fence、解释或 @提及。

JSON 形状：
{{"schemaVersion":1,"campaignId":"{id}","publishability":"publish|hold|no_content","reason":"...","audience":["..."],"coreMessage":"...","updates":[{{"id":"up-1","title":"...","summary":"...","userValue":"...","evidenceRefs":["ev-001"],"releaseState":"released|committed|unreleased"}}],"proofPoints":[{{"id":"proof-1","text":"...","evidenceRefs":["ev-001"]}}],"doNotClaim":["..."],"channelAngles":{{"xiaohongshu":"...","x":"...","zhihu":"...","bilibili":"...","github_release":"..."}}}}

Repository snapshot：
{snapshot}"#,
        id = campaign.id,
    )
}

fn writer_prompt(campaign: &ContentCampaign) -> AppResult<String> {
    let brief = campaign
        .brief
        .as_ref()
        .ok_or_else(|| "Campaign 缺少 Content Brief。".to_string())?;
    let brief_json = serde_json::to_string_pretty(brief).map_err(|e| e.to_string())?;
    let templates = campaign
        .snapshot
        .config
        .channel_templates
        .iter()
        .map(|(channel, body)| (channel, body.chars().take(1000).collect::<String>()))
        .collect::<BTreeMap<_, _>>();
    let config = serde_json::to_string_pretty(&json!({
        "brandGuide": campaign.snapshot.config.brand_guide.chars().take(1600).collect::<String>(),
        "channelTemplates": templates,
        "bannedPhrases": campaign.snapshot.config.banned_phrases,
    }))
    .map_err(|e| e.to_string())?;
    let revision = campaign
        .feedback
        .as_ref()
        .map(|feedback| {
            format!(
                "\n这是第 {} 次修改。用户意见：\n{}\n",
                campaign.revision, feedback
            )
        })
        .unwrap_or_default();
    Ok(format!(
        r#"【Self-Marketing / Channel Writer】
基于冻结的 Content Brief 生成五个渠道草稿。{revision}

硬规则：
1. 只能写 brief 里的事实；每个草稿 claimRefs 必须引用 brief 中 up-* / proof-* id。
2. 未发布内容必须明确使用“计划、开发中、尚未发布”等措辞。
3. 避免营销腔、空洞口号、绝对化承诺；优先具体变化、使用场景和证据边界。
4. X 正文最多 280 字符；GitHub Release 采用清晰的变更/验证/限制结构；B站输出口播脚本。
5. 恰好生成 xiaohongshu、x、zhihu、bilibili、github_release 五项。
6. 只输出一个 JSON 对象，不要 Markdown fence、解释或 @提及。

JSON 形状：
{{"schemaVersion":1,"campaignId":"{id}","drafts":[{{"channel":"xiaohongshu|x|zhihu|bilibili|github_release","title":"...","body":"...","claimRefs":["up-1","proof-1"]}}]}}

Content Brief：
{brief_json}

Brand / Channel config：
{config}"#,
        id = campaign.id,
    ))
}

pub fn expand_internal_prompt(conn: &Connection, root_message: &str) -> Option<String> {
    let (campaign_id, stage) = parse_internal_marker(root_message)?;
    let campaign = get_campaign(conn, campaign_id).ok()?;
    match stage {
        "planning" => Some(planner_prompt(&campaign)),
        "writing" | "revising" => writer_prompt(&campaign).ok(),
        _ => None,
    }
}

pub fn create_campaign(
    db_path: &Path,
    sched: SchedulerState,
    input: CreateCampaignInput,
) -> AppResult<ContentCampaign> {
    let source_mode = match input.source_mode.as_deref().unwrap_or("committed") {
        "committed" => "committed",
        "include_uncommitted" => "include_uncommitted",
        _ => return Err("sourceMode 只能是 committed 或 include_uncommitted。".into()),
    };
    let conn = open_db(db_path)?;
    let group = get_group(&conn, &input.group_id)?;
    if group.group_kind != "project" || group.workspace_path.trim().is_empty() {
        return Err("Self-Marketing 只支持绑定 Git 工作区的项目群。".into());
    }
    require_member(&conn, &group.id, &input.requested_by)?;
    let agents = usable_agents(&conn, &group.id)?;
    let planner_id = resolve_agent(
        &agents,
        input.planner_agent_id.as_deref(),
        &["content planner", "planner", "内容策划", "传播策划"],
        group.admin_member_id.as_deref(),
    )?;
    let writer_id = resolve_agent(
        &agents,
        input.writer_agent_id.as_deref(),
        &["channel writer", "writer", "内容写作", "文案"],
        Some(&planner_id),
    )?;
    drop(conn);

    let snapshot = collect_repository_snapshot(
        Path::new(&group.workspace_path),
        source_mode,
        input.base_ref.as_deref(),
    )?;
    let created_at = now();
    let mut campaign = ContentCampaign {
        id: id(),
        group_id: group.id.clone(),
        requested_by: input.requested_by,
        planner_agent_id: planner_id.clone(),
        writer_agent_id: writer_id,
        status: if snapshot.has_candidate_updates() {
            "planning"
        } else {
            "no_content"
        }
        .into(),
        source_mode: source_mode.into(),
        base_ref: snapshot.base_ref.clone(),
        head_ref: snapshot.head_ref.clone(),
        snapshot,
        brief: None,
        drafts: Vec::new(),
        validation: Vec::new(),
        planner_run_id: None,
        writer_run_id: None,
        revision: 0,
        feedback: None,
        feedback_by: None,
        error_message: None,
        approved_by: None,
        created_at,
        updated_at: created_at,
    };
    let conn = open_db(db_path)?;
    insert_campaign(&conn, &campaign)?;
    if campaign.status == "no_content" {
        campaign.error_message = Some("基准范围内没有新 commit；未显式包含未提交改动。".into());
        campaign.updated_at = now();
        save_campaign(&conn, &campaign)?;
        insert_card_message(&conn, &campaign, &campaign.requested_by)?;
        return Ok(campaign);
    }
    let run_id = insert_internal_run(&conn, &campaign, "planning", &planner_id)?;
    campaign.planner_run_id = Some(run_id);
    campaign.updated_at = now();
    save_campaign(&conn, &campaign)?;
    drop(conn);
    scheduler::schedule_group(sched, group.id);
    Ok(campaign)
}

fn extract_json<T: serde::de::DeserializeOwned>(raw: &str) -> AppResult<T> {
    let plain = parts_to_plain_text(raw);
    let start = plain
        .find('{')
        .ok_or_else(|| "Agent 输出中没有 JSON 对象。".to_string())?;
    let end = plain
        .rfind('}')
        .ok_or_else(|| "Agent 输出中的 JSON 不完整。".to_string())?;
    if end < start {
        return Err("Agent 输出中的 JSON 不完整。".into());
    }
    serde_json::from_str(&plain[start..=end]).map_err(|e| format!("Agent JSON 解析失败：{e}"))
}

fn has_errors(findings: &[ValidationFinding]) -> bool {
    findings.iter().any(|finding| finding.severity == "error")
}

fn replace_output_with_marker(
    conn: &Connection,
    state: &SchedulerState,
    group_id: &str,
    run_id: &str,
    message_id: &str,
    marker: &str,
) -> AppResult<()> {
    let current: String = conn
        .query_row(
            "SELECT content FROM messages WHERE id=?1",
            params![message_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let next = apply_channel_delta(&current, "final", marker, true);
    conn.execute(
        "UPDATE messages SET content=?1,status='completed' WHERE id=?2",
        params![next, message_id],
    )
    .map_err(|e| e.to_string())?;
    scheduler::emit(
        state,
        ChatEvent {
            kind: "message_delta".into(),
            group_id: group_id.into(),
            run_id: Some(run_id.into()),
            message_id: Some(message_id.into()),
            delta: Some(marker.into()),
            status: Some("completed".into()),
            error: None,
            channel: Some("final".into()),
            replace: Some(true),
            phase: None,
            elapsed_ms: None,
            total_ms: None,
            seq: None,
            delta_count: None,
            rss_mib: None,
        },
    );
    Ok(())
}

fn terminal_output(conn: &Connection, run_id: &str) -> AppResult<(String, String)> {
    conn.query_row(
        "SELECT r.output_message_id,m.content FROM task_runs r JOIN messages m ON m.id=r.output_message_id WHERE r.id=?1",
        params![run_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .map_err(|e| format!("读取 Self-Marketing Agent 输出失败：{e}"))
}

pub fn on_run_terminal(state: &SchedulerState, run_id: &str, succeeded: bool, error: Option<&str>) {
    let result = (|| -> AppResult<Option<String>> {
        let conn = open_db(&state.db_path)?;
        let Some((mut campaign, stage)) = find_by_run(&conn, run_id)? else {
            return Ok(None);
        };
        let (message_id, output) = terminal_output(&conn, run_id)?;
        if !succeeded {
            campaign.status = "failed".into();
            campaign.error_message =
                Some(error.unwrap_or("Self-Marketing Agent 运行失败。").into());
            campaign.updated_at = now();
            save_campaign(&conn, &campaign)?;
            replace_output_with_marker(
                &conn,
                state,
                &campaign.group_id,
                run_id,
                &message_id,
                &campaign_marker(&campaign.id),
            )?;
            return Ok(None);
        }
        if stage == "planning" {
            let brief: ContentBrief = extract_json(&output)?;
            if brief.campaign_id != campaign.id {
                return Err("Planner 返回的 campaignId 不匹配。".into());
            }
            let findings = validate_brief(&campaign.snapshot, &brief);
            campaign.brief = Some(brief);
            campaign.validation = findings;
            campaign.updated_at = now();
            if has_errors(&campaign.validation) {
                campaign.status = "failed".into();
                campaign.error_message = Some("Content Brief 未通过事实引用校验。".into());
                save_campaign(&conn, &campaign)?;
                replace_output_with_marker(
                    &conn,
                    state,
                    &campaign.group_id,
                    run_id,
                    &message_id,
                    &campaign_marker(&campaign.id),
                )?;
                return Ok(None);
            }
            let publishability = campaign
                .brief
                .as_ref()
                .map(|b| b.publishability.as_str())
                .unwrap_or("hold");
            if publishability != "publish" {
                campaign.status = "no_content".into();
                campaign.error_message = campaign.brief.as_ref().map(|b| b.reason.clone());
                save_campaign(&conn, &campaign)?;
                replace_output_with_marker(
                    &conn,
                    state,
                    &campaign.group_id,
                    run_id,
                    &message_id,
                    &campaign_marker(&campaign.id),
                )?;
                return Ok(None);
            }
            campaign.status = "writing".into();
            campaign.error_message = None;
            save_campaign(&conn, &campaign)?;
            replace_output_with_marker(
                &conn,
                state,
                &campaign.group_id,
                run_id,
                &message_id,
                &internal_marker(&campaign.id, "writing"),
            )?;
            let writer_id = campaign.writer_agent_id.clone();
            let writer_run = insert_internal_run(&conn, &campaign, "writing", &writer_id)?;
            campaign.writer_run_id = Some(writer_run);
            campaign.updated_at = now();
            save_campaign(&conn, &campaign)?;
            return Ok(Some(campaign.group_id));
        }

        let bundle: DraftBundle = extract_json(&output)?;
        if bundle.campaign_id != campaign.id || bundle.schema_version != 1 {
            return Err("Writer 返回的 campaignId 或 schemaVersion 不匹配。".into());
        }
        let brief = campaign
            .brief
            .as_ref()
            .ok_or_else(|| "Campaign 缺少 Content Brief。".to_string())?;
        let findings = validate_drafts(&campaign.snapshot, brief, &bundle.drafts);
        campaign.drafts = bundle.drafts;
        campaign.validation = findings;
        campaign.status = if has_errors(&campaign.validation) {
            "changes_requested"
        } else {
            "awaiting_user"
        }
        .into();
        campaign.error_message = if has_errors(&campaign.validation) {
            Some("渠道草稿未通过确定性校验，请修改后重试。".into())
        } else {
            None
        };
        campaign.updated_at = now();
        save_campaign(&conn, &campaign)?;
        replace_output_with_marker(
            &conn,
            state,
            &campaign.group_id,
            run_id,
            &message_id,
            &campaign_marker(&campaign.id),
        )?;
        Ok(None)
    })();
    match result {
        Ok(Some(group_id)) => scheduler::schedule_group(state.clone(), group_id),
        Ok(None) => {}
        Err(reason) => {
            if let Ok(conn) = open_db(&state.db_path) {
                if let Ok(Some((mut campaign, _))) = find_by_run(&conn, run_id) {
                    campaign.status = "failed".into();
                    campaign.error_message = Some(reason.clone());
                    campaign.updated_at = now();
                    let _ = save_campaign(&conn, &campaign);
                    if let Ok((message_id, _)) = terminal_output(&conn, run_id) {
                        let _ = replace_output_with_marker(
                            &conn,
                            state,
                            &campaign.group_id,
                            run_id,
                            &message_id,
                            &campaign_marker(&campaign.id),
                        );
                    }
                }
            }
            eprintln!("marketing on_run_terminal: {reason}");
        }
    }
}

pub fn revise_campaign(
    db_path: &Path,
    sched: SchedulerState,
    campaign_id: &str,
    input: ReviseCampaignInput,
) -> AppResult<ContentCampaign> {
    let conn = open_db(db_path)?;
    let mut campaign = get_campaign(&conn, campaign_id)?;
    require_member(&conn, &campaign.group_id, &input.actor_member_id)?;
    if !matches!(
        campaign.status.as_str(),
        "awaiting_user" | "changes_requested"
    ) {
        return Err("只有待审核或待修改的 Campaign 可以要求修改。".into());
    }
    let feedback = input.feedback.trim();
    if feedback.is_empty() {
        return Err("请填写具体修改意见。".into());
    }
    if feedback.chars().count() > 4_000 {
        return Err("修改意见不能超过 4000 字符。".into());
    }
    let agents = usable_agents(&conn, &campaign.group_id)?;
    if !agents
        .iter()
        .any(|agent| agent.id == campaign.writer_agent_id)
    {
        return Err("原 Writer 已不可用，请新建 Campaign 并选择新的 Agent。".into());
    }
    campaign.status = "writing".into();
    campaign.revision += 1;
    campaign.feedback = Some(feedback.into());
    campaign.feedback_by = Some(input.actor_member_id.clone());
    campaign.validation.clear();
    campaign.error_message = None;
    let writer_id = campaign.writer_agent_id.clone();
    let run_id = insert_internal_run(&conn, &campaign, "revising", &writer_id)?;
    campaign.writer_run_id = Some(run_id);
    campaign.updated_at = now();
    save_campaign(&conn, &campaign)?;
    let group_id = campaign.group_id.clone();
    drop(conn);
    scheduler::schedule_group(sched, group_id);
    Ok(campaign)
}

pub fn approve_campaign(
    db_path: &Path,
    campaign_id: &str,
    input: ApproveCampaignInput,
) -> AppResult<ContentCampaign> {
    let conn = open_db(db_path)?;
    let mut campaign = get_campaign(&conn, campaign_id)?;
    require_member(&conn, &campaign.group_id, &input.actor_member_id)?;
    let group = get_group(&conn, &campaign.group_id)?;
    let can_approve = input.actor_member_id == campaign.requested_by
        || input.actor_member_id == group.owner_member_id
        || group.admin_member_id.as_deref() == Some(input.actor_member_id.as_str());
    if !can_approve {
        return Err("只有发起人、群主或群管理员可以批准宣传内容。".into());
    }
    if campaign.status != "awaiting_user" {
        return Err("Campaign 尚未进入可批准状态。".into());
    }
    let brief = campaign
        .brief
        .as_ref()
        .ok_or_else(|| "Campaign 缺少 Content Brief。".to_string())?;
    campaign.validation = validate_drafts(&campaign.snapshot, brief, &campaign.drafts);
    if has_errors(&campaign.validation) {
        campaign.status = "changes_requested".into();
        campaign.error_message = Some("批准前复检失败，请先修改草稿。".into());
        campaign.updated_at = now();
        save_campaign(&conn, &campaign)?;
        return Err("批准前复检失败，请先处理阻断项。".into());
    }
    campaign.status = "approved".into();
    campaign.approved_by = Some(input.actor_member_id.clone());
    campaign.error_message = None;
    campaign.updated_at = now();
    save_campaign(&conn, &campaign)?;
    insert_card_message(&conn, &campaign, &input.actor_member_id)?;
    Ok(campaign)
}

pub fn export_campaign(conn: &Connection, campaign_id: &str) -> AppResult<CampaignExport> {
    let campaign = get_campaign(conn, campaign_id)?;
    if campaign.status != "approved" {
        return Err("只有已批准的 Campaign 可以导出。".into());
    }
    let brief = campaign
        .brief
        .as_ref()
        .ok_or_else(|| "Campaign 缺少 Content Brief。".to_string())?;
    let mut markdown = format!(
        "# Self-Marketing Campaign {}\n\n- Head: `{}`\n- Base: `{}`\n- Source mode: `{}`\n- Approved by: `{}`\n\n## Content Brief\n\n**Core message:** {}\n\n**Audience:** {}\n\n**Why:** {}\n\n",
        campaign.id,
        campaign.head_ref,
        campaign.base_ref.as_deref().unwrap_or("(initial history)"),
        campaign.source_mode,
        campaign.approved_by.as_deref().unwrap_or("unknown"),
        brief.core_message,
        brief.audience.join("、"),
        brief.reason,
    );
    markdown.push_str("### Updates\n\n");
    for update in &brief.updates {
        markdown.push_str(&format!(
            "- **{}** — {}\n  - 用户价值：{}\n  - 状态：{}\n  - 证据：{}\n",
            update.title,
            update.summary,
            update.user_value,
            update.release_state,
            update.evidence_refs.join(", ")
        ));
    }
    for channel in REQUIRED_CHANNELS {
        if let Some(draft) = campaign
            .drafts
            .iter()
            .find(|draft| draft.channel == channel)
        {
            markdown.push_str(&format!(
                "\n## {}\n\n### {}\n\n{}\n\n_Claim refs: {}_\n",
                channel,
                draft.title,
                draft.body,
                draft.claim_refs.join(", ")
            ));
        }
    }
    markdown.push_str("\n## Evidence index\n\n");
    for item in &campaign.snapshot.evidence {
        markdown.push_str(&format!(
            "- `{}` {} — `{}` — {}\n",
            item.id, item.kind, item.source, item.content_hash
        ));
    }
    Ok(CampaignExport {
        filename: format!("self-marketing-{}.md", campaign.id),
        markdown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_sender::EventSender;
    use std::collections::{HashMap, HashSet};
    use std::process::Command;
    use std::sync::{Arc, Mutex};
    use tokio::sync::broadcast;

    #[test]
    fn internal_marker_round_trip() {
        let marker = internal_marker("campaign-1", "planning");
        assert_eq!(
            parse_internal_marker(&marker),
            Some(("campaign-1", "planning"))
        );
        assert!(parse_internal_marker("hello").is_none());
    }

    #[test]
    fn extract_json_accepts_fenced_agent_output() {
        let raw = "```json\n{\"value\":1}\n```";
        let value: serde_json::Value = extract_json(raw).unwrap();
        assert_eq!(value["value"], 1);
    }

    #[test]
    fn no_content_campaign_round_trips_without_starting_an_agent() {
        let repo = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            assert!(Command::new("git")
                .args(args)
                .current_dir(repo.path())
                .status()
                .unwrap()
                .success());
        };
        git(&["init"]);
        git(&["config", "user.email", "marketing@example.com"]);
        git(&["config", "user.name", "Marketing Test"]);
        std::fs::write(repo.path().join("README.md"), "# Test project").unwrap();
        git(&["add", "README.md"]);
        git(&["commit", "-m", "feat: initial project"]);

        let db = tempfile::NamedTempFile::new().unwrap();
        crate::db::init_db(db.path()).unwrap();
        let conn = open_db(db.path()).unwrap();
        conn.execute(
            "UPDATE groups SET workspace_path=?1 WHERE id='seed-group-ohmyworkpanel'",
            params![repo.path().to_string_lossy()],
        )
        .unwrap();
        drop(conn);
        let (tx, _rx) = broadcast::channel(8);
        let sched = SchedulerState {
            db_path: db.path().to_path_buf(),
            event_sender: EventSender::Web(tx),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
            scheduling_groups: Arc::new(Mutex::new(HashSet::new())),
            live_sessions: Arc::new(Mutex::new(HashMap::new())),
        };
        let campaign = create_campaign(
            db.path(),
            sched,
            CreateCampaignInput {
                group_id: "seed-group-ohmyworkpanel".into(),
                requested_by: "seed-member-owner-root".into(),
                planner_agent_id: Some("seed-member-codex".into()),
                writer_agent_id: Some("seed-member-codex".into()),
                source_mode: Some("committed".into()),
                base_ref: Some("HEAD".into()),
            },
        )
        .unwrap();
        assert_eq!(campaign.status, "no_content");
        assert!(campaign.planner_run_id.is_none());
        let stored = get_campaign(&open_db(db.path()).unwrap(), &campaign.id).unwrap();
        assert_eq!(stored.head_ref, campaign.head_ref);
        assert!(stored.feedback_by.is_none());
    }
}
