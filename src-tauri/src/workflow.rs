//! ohMyWorkPanel V1.3.0 workflow: versions / roadmap / waves / ask helpers.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::db::{id, now, open_db, AppResult};
use crate::git_inspect::{inspect_workspace, GitSnapshot};
use crate::models::Group;
use std::path::Path;

pub fn ensure_workflow_tables(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS project_versions (
          id TEXT PRIMARY KEY,
          group_id TEXT NOT NULL,
          name TEXT NOT NULL,
          git_tag TEXT,
          git_sha TEXT,
          kind TEXT NOT NULL,
          status TEXT NOT NULL,
          what TEXT NOT NULL DEFAULT '',
          who TEXT NOT NULL DEFAULT '',
          how TEXT NOT NULL DEFAULT '',
          one_liner TEXT NOT NULL DEFAULT '',
          requester_member_id TEXT,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          released_at INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_pv_group ON project_versions(group_id, created_at DESC);
        CREATE TABLE IF NOT EXISTS waves (
          id TEXT PRIMARY KEY,
          version_id TEXT NOT NULL,
          group_id TEXT NOT NULL,
          idx INTEGER NOT NULL,
          title TEXT NOT NULL,
          status TEXT NOT NULL,
          phase TEXT NOT NULL DEFAULT 'assign',
          phase_cursor TEXT NOT NULL DEFAULT '',
          play_state TEXT NOT NULL DEFAULT 'paused',
          summary TEXT NOT NULL DEFAULT '',
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_waves_version ON waves(version_id, idx);
        "#,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectVersion {
    pub id: String,
    pub group_id: String,
    pub name: String,
    pub git_tag: Option<String>,
    pub git_sha: Option<String>,
    pub kind: String,
    pub status: String,
    pub what: String,
    pub who: String,
    pub how: String,
    pub one_liner: String,
    pub requester_member_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub released_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wave {
    pub id: String,
    pub version_id: String,
    pub group_id: String,
    pub idx: i64,
    pub title: String,
    pub status: String,
    pub phase: String,
    pub phase_cursor: String,
    pub play_state: String,
    pub summary: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionBoard {
    pub git: GitSnapshot,
    pub versions: Vec<ProjectVersion>,
    pub waves: Vec<Wave>,
    pub asking_version_id: Option<String>,
    pub admin_member_id: Option<String>,
    /// Group workspace used for Git tag timeline (not the same as version rows).
    pub workspace_path: String,
    /// Other project groups that share this workspace (Git looks identical).
    pub workspace_shared_with: Vec<String>,
}

fn version_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectVersion> {
    Ok(ProjectVersion {
        id: row.get(0)?,
        group_id: row.get(1)?,
        name: row.get(2)?,
        git_tag: row.get(3)?,
        git_sha: row.get(4)?,
        kind: row.get(5)?,
        status: row.get(6)?,
        what: row.get(7)?,
        who: row.get(8)?,
        how: row.get(9)?,
        one_liner: row.get(10)?,
        requester_member_id: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
        released_at: row.get(14)?,
    })
}

const VERSION_SELECT: &str = "SELECT id,group_id,name,git_tag,git_sha,kind,status,what,who,how,one_liner,requester_member_id,created_at,updated_at,released_at FROM project_versions";

fn wave_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Wave> {
    Ok(Wave {
        id: row.get(0)?,
        version_id: row.get(1)?,
        group_id: row.get(2)?,
        idx: row.get(3)?,
        title: row.get(4)?,
        status: row.get(5)?,
        phase: row.get(6)?,
        phase_cursor: row.get(7)?,
        play_state: row.get(8)?,
        summary: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

const WAVE_SELECT: &str = "SELECT id,version_id,group_id,idx,title,status,phase,phase_cursor,play_state,summary,created_at,updated_at FROM waves";

pub fn list_versions(conn: &Connection, group_id: &str) -> AppResult<Vec<ProjectVersion>> {
    let mut stmt = conn
        .prepare(&format!(
            "{VERSION_SELECT} WHERE group_id=?1 ORDER BY created_at DESC"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![group_id], version_from_row)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn get_version(conn: &Connection, id: &str) -> AppResult<ProjectVersion> {
    conn.query_row(
        &format!("{VERSION_SELECT} WHERE id=?1"),
        params![id],
        version_from_row,
    )
    .map_err(|_| "版本不存在".into())
}

pub fn list_waves_for_group(conn: &Connection, group_id: &str) -> AppResult<Vec<Wave>> {
    let mut stmt = conn
        .prepare(&format!(
            "{WAVE_SELECT} WHERE group_id=?1 ORDER BY version_id, idx"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![group_id], wave_from_row)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn list_waves(conn: &Connection, version_id: &str) -> AppResult<Vec<Wave>> {
    let mut stmt = conn
        .prepare(&format!(
            "{WAVE_SELECT} WHERE version_id=?1 ORDER BY idx"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![version_id], wave_from_row)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn get_wave(conn: &Connection, id: &str) -> AppResult<Wave> {
    conn.query_row(
        &format!("{WAVE_SELECT} WHERE id=?1"),
        params![id],
        wave_from_row,
    )
    .map_err(|_| "Wave 不存在".into())
}

pub fn asking_version_id(conn: &Connection, group_id: &str) -> AppResult<Option<String>> {
    conn.query_row(
        "SELECT id FROM project_versions WHERE group_id=?1 AND status='asking' ORDER BY updated_at DESC LIMIT 1",
        params![group_id],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

/// Other non-chat groups that use the same workspace_path (Git timeline will match).
pub fn workspace_peer_group_names(
    conn: &Connection,
    group_id: &str,
    workspace_path: &str,
) -> AppResult<Vec<String>> {
    let ws = workspace_path.trim();
    if ws.is_empty() {
        return Ok(Vec::new());
    }
    let mut stmt = conn
        .prepare(
            "SELECT name FROM groups
             WHERE id != ?1
               AND trim(ifnull(workspace_path,'')) = ?2
               AND ifnull(group_kind,'project') != 'chat'
             ORDER BY name COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![group_id, ws], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let mut names = Vec::new();
    for row in rows {
        names.push(row.map_err(|e| e.to_string())?);
    }
    Ok(names)
}

pub fn board_for_group(db_path: &Path, group: &Group) -> AppResult<VersionBoard> {
    let conn = open_db(db_path)?;
    ensure_workflow_tables(&conn)?;
    let git = inspect_workspace(&group.workspace_path);
    let versions = list_versions(&conn, &group.id)?;
    let waves = list_waves_for_group(&conn, &group.id)?;
    let asking = asking_version_id(&conn, &group.id)?;
    let workspace_shared_with =
        workspace_peer_group_names(&conn, &group.id, &group.workspace_path)?;
    Ok(VersionBoard {
        git,
        versions,
        waves,
        asking_version_id: asking,
        admin_member_id: group.admin_member_id.clone(),
        workspace_path: group.workspace_path.clone(),
        workspace_shared_with,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateVersionInput {
    pub group_id: String,
    pub name: Option<String>,
    pub what: Option<String>,
    pub who: Option<String>,
    pub how: Option<String>,
    pub one_liner: Option<String>,
    pub requester_member_id: Option<String>,
    /// "create" | "import"
    pub mode: Option<String>,
}

pub fn create_version(
    conn: &Connection,
    group: &Group,
    input: &CreateVersionInput,
) -> AppResult<ProjectVersion> {
    ensure_workflow_tables(conn)?;
    if group.admin_member_id.as_deref().unwrap_or("").is_empty() {
        return Err("请先设置群管理员 Agent，才能新建/导入版本".into());
    }
    let git = inspect_workspace(&group.workspace_path);
    let mode = input.mode.as_deref().unwrap_or("create");
    let ts = now();
    let vid = id();

    let (name, kind, git_tag, git_sha, status, what, who, how, one_liner) = if mode == "import" {
        let tag = git.tags.first();
        let name = input
            .name
            .clone()
            .or_else(|| tag.map(|t| t.name.clone()))
            .unwrap_or_else(|| "imported".into());
        let kind = if tag.map(|t| t.is_virtual).unwrap_or(true) {
            "virtual"
        } else {
            "tag"
        };
        let what = input.what.clone().unwrap_or_else(|| {
            format!(
                "从 git 导入：{}\n最近提交：{}",
                name,
                git.recent_commits
                    .iter()
                    .take(5)
                    .map(|c| format!("{} {}", c.sha, c.subject))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
        (
            name,
            kind.to_string(),
            tag.map(|t| t.name.clone()),
            tag.map(|t| t.sha.clone()).or(git.head_sha.clone()),
            "planning".to_string(),
            what,
            input.who.clone().unwrap_or_default(),
            input.how.clone().unwrap_or_default(),
            input.one_liner.clone().unwrap_or_default(),
        )
    } else {
        let name = input
            .name
            .clone()
            .unwrap_or_else(|| suggest_next_version_name(&git));
        (
            name,
            "draft".into(),
            None,
            git.head_sha.clone(),
            "planning".into(),
            input.what.clone().unwrap_or_default(),
            input.who.clone().unwrap_or_default(),
            input.how.clone().unwrap_or_default(),
            input.one_liner.clone().unwrap_or_default(),
        )
    };

    conn.execute(
        "INSERT INTO project_versions(id,group_id,name,git_tag,git_sha,kind,status,what,who,how,one_liner,requester_member_id,created_at,updated_at,released_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,NULL)",
        params![
            vid,
            group.id,
            name,
            git_tag,
            git_sha,
            kind,
            status,
            what,
            who,
            how,
            one_liner,
            input.requester_member_id,
            ts,
            ts
        ],
    )
    .map_err(|e| e.to_string())?;
    get_version(conn, &vid)
}

fn suggest_next_version_name(git: &GitSnapshot) -> String {
    for t in &git.tags {
        if t.is_virtual {
            continue;
        }
        if let Some(rest) = t.name.strip_prefix('v') {
            let parts: Vec<_> = rest.split('.').collect();
            if parts.len() >= 2 {
                if let (Ok(maj), Ok(min)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                    return format!("v{}.{}", maj, min + 1);
                }
            }
        }
    }
    "v0.1.0".into()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRoadmapInput {
    pub what: Option<String>,
    pub who: Option<String>,
    pub how: Option<String>,
    pub one_liner: Option<String>,
    pub name: Option<String>,
    pub requester_member_id: Option<String>,
}

pub fn update_roadmap(
    conn: &Connection,
    version_id: &str,
    input: &UpdateRoadmapInput,
) -> AppResult<ProjectVersion> {
    let mut v = get_version(conn, version_id)?;
    if let Some(x) = &input.what {
        v.what = x.clone();
    }
    if let Some(x) = &input.who {
        v.who = x.clone();
    }
    if let Some(x) = &input.how {
        v.how = x.clone();
    }
    if let Some(x) = &input.one_liner {
        v.one_liner = x.clone();
    }
    if let Some(x) = &input.name {
        v.name = x.clone();
    }
    if let Some(x) = &input.requester_member_id {
        v.requester_member_id = Some(x.clone());
    }
    let ts = now();
    conn.execute(
        "UPDATE project_versions SET what=?1, who=?2, how=?3, one_liner=?4, name=?5, requester_member_id=?6, updated_at=?7 WHERE id=?8",
        params![
            v.what,
            v.who,
            v.how,
            v.one_liner,
            v.name,
            v.requester_member_id,
            ts,
            version_id
        ],
    )
    .map_err(|e| e.to_string())?;
    get_version(conn, version_id)
}

pub fn start_ask(conn: &Connection, group: &Group, version_id: &str) -> AppResult<ProjectVersion> {
    if group.admin_member_id.as_deref().unwrap_or("").is_empty() {
        return Err("没有管理员 Agent，无法进入 Ask 模式".into());
    }
    let v = get_version(conn, version_id)?;
    if v.group_id != group.id {
        return Err("版本不属于本群".into());
    }
    // clear other asking
    conn.execute(
        "UPDATE project_versions SET status='planning', updated_at=?1 WHERE group_id=?2 AND status='asking'",
        params![now(), group.id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE project_versions SET status='asking', updated_at=?1 WHERE id=?2",
        params![now(), version_id],
    )
    .map_err(|e| e.to_string())?;
    get_version(conn, version_id)
}

pub fn cancel_ask(conn: &Connection, version_id: &str) -> AppResult<ProjectVersion> {
    conn.execute(
        "UPDATE project_versions SET status='planning', updated_at=?1 WHERE id=?2 AND status='asking'",
        params![now(), version_id],
    )
    .map_err(|e| e.to_string())?;
    get_version(conn, version_id)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveWavesInput {
    pub waves: Vec<ApproveWaveItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveWaveItem {
    pub title: String,
}

pub fn approve_waves(
    conn: &Connection,
    version_id: &str,
    input: &ApproveWavesInput,
) -> AppResult<(ProjectVersion, Vec<Wave>)> {
    let v = get_version(conn, version_id)?;
    if input.waves.is_empty() {
        return Err("至少需要一个 Wave".into());
    }
    // replace waves
    conn.execute("DELETE FROM waves WHERE version_id=?1", params![version_id])
        .map_err(|e| e.to_string())?;
    let ts = now();
    for (i, w) in input.waves.iter().enumerate() {
        conn.execute(
            "INSERT INTO waves(id,version_id,group_id,idx,title,status,phase,phase_cursor,play_state,summary,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,'pending','assign','','paused','',?6,?7)",
            params![id(), version_id, v.group_id, (i + 1) as i64, w.title, ts, ts],
        )
        .map_err(|e| e.to_string())?;
    }
    conn.execute(
        "UPDATE project_versions SET status='ready', updated_at=?1 WHERE id=?2",
        params![ts, version_id],
    )
    .map_err(|e| e.to_string())?;
    Ok((get_version(conn, version_id)?, list_waves(conn, version_id)?))
}

/// Default wave titles from roadmap one-liner / what (S3 helper when agent hasn't proposed).
pub fn default_waves_from_roadmap(v: &ProjectVersion) -> Vec<ApproveWaveItem> {
    let base = if !v.one_liner.trim().is_empty() {
        v.one_liner.trim().to_string()
    } else if !v.what.trim().is_empty() {
        v.what.lines().next().unwrap_or("迭代").trim().to_string()
    } else {
        "核心能力".into()
    };
    vec![
        ApproveWaveItem {
            title: format!("Wave1 · 澄清与设计（{base}）"),
        },
        ApproveWaveItem {
            title: format!("Wave2 · 实现与验收（{base}）"),
        },
    ]
}

pub const WAVE_PHASES: &[&str] = &[
    "assign",
    "clarify",
    "design",
    "develop",
    "verify",
    "summary",
];

pub fn phase_label(phase: &str) -> String {
    match phase {
        "assign" => "原始需求分配".into(),
        "clarify" => "需求澄清".into(),
        "design" => "需求设计".into(),
        "develop" => "迭代开发".into(),
        "verify" => "测试灰度验收".into(),
        "summary" => "总结".into(),
        other => other.to_string(),
    }
}

pub fn play_wave(conn: &Connection, wave_id: &str) -> AppResult<Wave> {
    let w = get_wave(conn, wave_id)?;
    conn.execute(
        "UPDATE waves SET play_state='playing', status='running', updated_at=?1 WHERE id=?2",
        params![now(), wave_id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE project_versions SET status='wave_running', updated_at=?1 WHERE id=?2",
        params![now(), w.version_id],
    )
    .map_err(|e| e.to_string())?;
    get_wave(conn, wave_id)
}

pub fn pause_wave(conn: &Connection, wave_id: &str) -> AppResult<Wave> {
    conn.execute(
        "UPDATE waves SET play_state='paused', status='paused', updated_at=?1 WHERE id=?2",
        params![now(), wave_id],
    )
    .map_err(|e| e.to_string())?;
    get_wave(conn, wave_id)
}

pub fn advance_wave_phase(conn: &Connection, wave_id: &str) -> AppResult<Wave> {
    let w = get_wave(conn, wave_id)?;
    let idx = WAVE_PHASES.iter().position(|p| *p == w.phase).unwrap_or(0);
    if idx + 1 >= WAVE_PHASES.len() {
        conn.execute(
            "UPDATE waves SET status='done', play_state='paused', phase='summary', updated_at=?1 WHERE id=?2",
            params![now(), wave_id],
        )
        .map_err(|e| e.to_string())?;
        // if all waves done → awaiting_release
        let pending: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM waves WHERE version_id=?1 AND status!='done' AND status!='skipped'",
                params![w.version_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if pending == 0 {
            conn.execute(
                "UPDATE project_versions SET status='awaiting_release', updated_at=?1 WHERE id=?2",
                params![now(), w.version_id],
            )
            .map_err(|e| e.to_string())?;
        }
    } else {
        let next = WAVE_PHASES[idx + 1];
        conn.execute(
            "UPDATE waves SET phase=?1, updated_at=?2 WHERE id=?3",
            params![next, now(), wave_id],
        )
        .map_err(|e| e.to_string())?;
    }
    get_wave(conn, wave_id)
}

pub fn play_version_roadmap(conn: &Connection, version_id: &str) -> AppResult<Option<Wave>> {
    let waves = list_waves(conn, version_id)?;
    let next = waves
        .into_iter()
        .find(|w| w.status != "done" && w.status != "skipped");
    if let Some(w) = next {
        Ok(Some(play_wave(conn, &w.id)?))
    } else {
        conn.execute(
            "UPDATE project_versions SET status='awaiting_release', updated_at=?1 WHERE id=?2",
            params![now(), version_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(None)
    }
}

pub fn pause_version_roadmap(conn: &Connection, version_id: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE waves SET play_state='paused', status=CASE WHEN status='running' THEN 'paused' ELSE status END, updated_at=?1 WHERE version_id=?2 AND play_state='playing'",
        params![now(), version_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn mark_released(conn: &Connection, version_id: &str, git_tag: Option<String>) -> AppResult<ProjectVersion> {
    let ts = now();
    conn.execute(
        "UPDATE project_versions SET status='released', released_at=?1, updated_at=?1, git_tag=COALESCE(?2, git_tag), kind=CASE WHEN ?2 IS NOT NULL THEN 'tag' ELSE kind END WHERE id=?3",
        params![ts, git_tag, version_id],
    )
    .map_err(|e| e.to_string())?;
    get_version(conn, version_id)
}

/// Ask-gate: when group has asking version, only allow runs for admin from requester/owner/@admin/A2A.
pub fn ask_allows_agent_run(
    conn: &Connection,
    group_id: &str,
    sender_member_id: &str,
    target_agent_id: &str,
    mention_ids: &[String],
    is_a2a: bool,
) -> AppResult<bool> {
    ensure_workflow_tables(conn)?;
    let asking = asking_version_id(conn, group_id)?;
    let Some(vid) = asking else {
        return Ok(true);
    };
    let v = get_version(conn, &vid)?;
    let group_admin: Option<String> = conn
        .query_row(
            "SELECT admin_member_id FROM groups WHERE id=?1",
            params![group_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .flatten();
    let Some(admin_id) = group_admin else {
        return Ok(true);
    };
    // Only gate the admin agent
    if target_agent_id != admin_id {
        return Ok(true);
    }
    if is_a2a {
        return Ok(true);
    }
    if mention_ids.iter().any(|m| m == &admin_id) && sender_member_id != admin_id {
        // explicit @admin from anyone — allow interrupt
        // but design says only requester/owner or @admin — allow @admin
        return Ok(true);
    }
    let owner: String = conn
        .query_row(
            "SELECT owner_member_id FROM groups WHERE id=?1",
            params![group_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if sender_member_id == owner {
        return Ok(true);
    }
    if v.requester_member_id.as_deref() == Some(sender_member_id) {
        return Ok(true);
    }
    Ok(false)
}

pub fn build_ask_kickoff_prompt(v: &ProjectVersion) -> String {
    format!(
        "【版本 Ask 模式 · {}】\n你已进入 Ask 模式，请主动 @需求提出人澄清 What / Who / How。\n\
         一句话：{}\nWhat：{}\nWho：{}\nHow：{}\n\
         澄清完成后，请给出 Wave1…WaveN 拆分方案，等待需求提出人同意。\n\
         同意后可在版本页点「确认 Waves」或让平台按默认两 Wave 落库。\n\
         Ask 期间请优先只与需求提出人对话；忽略无关闲聊。",
        v.name,
        v.one_liner,
        v.what,
        v.who,
        v.how
    )
}

pub fn build_wave_kickoff_prompt(v: &ProjectVersion, w: &Wave) -> String {
    format!(
        "【Wave 执行 · {} / {} · 阶段：{}】\n\
         请按序执行：原始需求分配 → 需求澄清 → 需求设计 → 迭代开发 → 测试灰度验收 → 总结。\n\
         当前阶段：{}（{}）\n\
         版本 Roadmap：What={} | Who={} | How={}\n\
         有 Codex 时开发阶段可用 Codex；否则用可用 Agent。完成后在版本页推进阶段或继续下一 Wave。\n\
         播放状态：{}。若用户暂停请停止开新任务。",
        v.name,
        w.title,
        w.phase,
        phase_label(&w.phase),
        w.phase,
        v.what,
        v.who,
        v.how,
        w.play_state
    )
}

/// 处理 / 斜杠命令（项目群 + 用户成员；纯 conn，不产生 run）。
/// 返回 Some(回显文本) 表示已处理；None 表示普通消息（含未知命令，避免误伤）。
pub fn try_slash_command(
    conn: &Connection,
    group_id: &str,
    group_kind: &str,
    member_kind: &str,
    content: &str,
) -> AppResult<Option<String>> {
    if group_kind != "project" || member_kind != "user" {
        return Ok(None);
    }
    let trimmed = content.trim();
    let Some(rest) = trimmed.strip_prefix('/') else {
        return Ok(None);
    };
    if rest.is_empty() {
        return Ok(None);
    }
    let (cmd, arg) = match rest.split_once(char::is_whitespace) {
        Some((c, a)) => (c.trim(), a.trim()),
        None => (rest, ""),
    };
    match cmd {
        "board" => {
            let mut versions = list_versions(conn, group_id)?;
            versions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            let Some(v) = versions.first() else {
                return Ok(Some("当前没有已建立的版本。".into()));
            };
            let waves = list_waves(conn, &v.id)?;
            let done = waves
                .iter()
                .filter(|w| w.status == "done" || w.status == "skipped")
                .count();
            let mut text = format!("版本 {}（{}）", v.name, v.status);
            if let Some(w) = waves.iter().find(|w| w.status == "running" || w.status == "paused") {
                text.push_str(&format!(
                    "\n→ Wave {}{}：{}",
                    w.idx,
                    if w.status == "running" { " · 进行中" } else { " · 已暂停" },
                    w.title
                ));
            } else if !waves.is_empty() {
                text.push_str(&format!("\n→ 无进行中 Wave（完成 {}/{}）", done, waves.len()));
            }
            Ok(Some(text))
        }
        "approve" => {
            let versions = list_versions(conn, group_id)?;
            let Some(v) = versions.iter().find(|v| v.status == "asking") else {
                return Ok(Some("没有处于 Ask 待批准的版本。".into()));
            };
            let input = ApproveWavesInput { waves: default_waves_from_roadmap(v) };
            let (version, waves) = approve_waves(conn, &v.id, &input)?;
            let titles: Vec<&str> = waves.iter().map(|w| w.title.as_str()).collect();
            Ok(Some(format!("已批准版本 {}，生成 {} 个 Wave：{}", version.name, waves.len(), titles.join(" / "))))
        }
        "wave" => {
            if arg.is_empty() {
                return Ok(Some("用法：/wave <Wave 标题>".into()));
            }
            let mut versions = list_versions(conn, group_id)?;
            versions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            let Some(v) = versions.first() else {
                return Ok(Some("还没有版本，无法创建 Wave。".into()));
            };
            let input = ApproveWavesInput { waves: vec![ApproveWaveItem { title: arg.to_string() }] };
            let (version, waves) = approve_waves(conn, &v.id, &input)?;
            let titles: Vec<&str> = waves.iter().map(|w| w.title.as_str()).collect();
            Ok(Some(format!("版本 {} 的 Wave 已更新为 {} 个：{}", version.name, waves.len(), titles.join(" / "))))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use tempfile::NamedTempFile;

    #[test]
    fn slash_command_board_wave_approve() {
        let file = NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        ensure_workflow_tables(&conn).unwrap();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at,group_kind) VALUES('g','g','.','o','admin',1,'project')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,tags) VALUES('o','g','user','o','#000','',1,1,'')",
            [],
        )
        .unwrap();
        let group = crate::db::get_group(&conn, "g").unwrap();
        let v = create_version(
            &conn,
            &group,
            &CreateVersionInput {
                group_id: "g".into(),
                name: Some("v0.1.0".into()),
                what: Some("做版本页".into()),
                who: Some("团队".into()),
                how: Some("分两波".into()),
                one_liner: Some("版本页 MVP".into()),
                requester_member_id: Some("o".into()),
                mode: Some("create".into()),
            },
        )
        .unwrap();

        // 群类型 / 成员类型 / 未知命令 → None（不劫持普通消息）
        assert!(try_slash_command(&conn, "g", "chat", "user", "/board").unwrap().is_none());
        assert!(try_slash_command(&conn, "g", "project", "agent", "/board").unwrap().is_none());
        assert!(try_slash_command(&conn, "g", "project", "user", "/frobnicate").unwrap().is_none());

        // /board → 摘要
        let b = try_slash_command(&conn, "g", "project", "user", "/board").unwrap().unwrap();
        assert!(b.contains("v0.1.0"));

        // /wave <title> → 生成 1 个 Wave 并置 ready
        let w = try_slash_command(&conn, "g", "project", "user", "/wave 第一波").unwrap().unwrap();
        assert!(w.contains("第一波"));
        assert_eq!(list_waves(&conn, &v.id).unwrap().len(), 1);

        // /approve 无 asking → 提示
        let a = try_slash_command(&conn, "g", "project", "user", "/approve").unwrap().unwrap();
        assert!(a.contains("没有处于 Ask"));
    }

    #[test]
    fn create_and_approve_waves() {
        let file = NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        ensure_workflow_tables(&conn).unwrap();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES('g','g','.','o','admin',1)",
            [],
        )
        .unwrap();
        let group = crate::db::get_group(&conn, "g").unwrap();
        let v = create_version(
            &conn,
            &group,
            &CreateVersionInput {
                group_id: "g".into(),
                name: Some("v0.1.0".into()),
                what: Some("做版本页".into()),
                who: Some("团队".into()),
                how: Some("分两波".into()),
                one_liner: Some("版本页 MVP".into()),
                requester_member_id: Some("o".into()),
                mode: Some("create".into()),
            },
        )
        .unwrap();
        assert_eq!(v.status, "planning");
        start_ask(&conn, &group, &v.id).unwrap();
        assert_eq!(asking_version_id(&conn, "g").unwrap().as_deref(), Some(v.id.as_str()));
        assert!(!ask_allows_agent_run(&conn, "g", "stranger", "admin", &[], false).unwrap());
        assert!(ask_allows_agent_run(&conn, "g", "o", "admin", &[], false).unwrap());
        let (_v2, waves) = approve_waves(
            &conn,
            &v.id,
            &ApproveWavesInput {
                waves: default_waves_from_roadmap(&v),
            },
        )
        .unwrap();
        assert_eq!(waves.len(), 2);
        let w = play_wave(&conn, &waves[0].id).unwrap();
        assert_eq!(w.play_state, "playing");
        pause_wave(&conn, &w.id).unwrap();
    }

    #[test]
    fn workspace_peers_list_other_project_groups() {
        let file = NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at,group_kind)
             VALUES('g1','Alpha','/AI/Shared','o','admin',1,'project')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at,group_kind)
             VALUES('g2','Beta','/AI/Shared','o','admin',2,'project')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at,group_kind)
             VALUES('g3','ChatOnly','/AI/Shared','o','admin',3,'chat')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at,group_kind)
             VALUES('g4','Other','/AI/Else','o','admin',4,'project')",
            [],
        )
        .unwrap();
        let peers = workspace_peer_group_names(&conn, "g1", "/AI/Shared").unwrap();
        assert_eq!(peers, vec!["Beta".to_string()]);
        let board = board_for_group(file.path(), &crate::db::get_group(&conn, "g1").unwrap()).unwrap();
        assert_eq!(board.workspace_path, "/AI/Shared");
        assert_eq!(board.workspace_shared_with, vec!["Beta".to_string()]);
    }
}
