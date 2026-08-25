//! Agent 配置一键导入 / 导出 / 环境自检 / CLI 自动安装。
//!
//! 目的：把「ECS 上 vibecoding 配好的 Agent 环境」打包成一份可移植配置包，
//! 在本地 / 新安装上一键导入并落盘（`~/.codex`、`~/.claude`、`~/.cursor`、
//! 以及通用 `files` 逃生口），同时同步成员（`agent_profiles`）与持久化启动重放，
//! 从而做到**开箱即用**：新增用户不再需要重新 vibecoding。
//!
//! - 导出：`GET /api/agent-config/status` 自带有效配置（脱敏）；`POST /api/agent-config/export`
//!   生成完整配置包（可选含密钥）。
//! - 导入：`POST /api/agent-config/import` 一键应用 +（可选）自动安装缺失 CLI。
//! - 启动重放：`main_server` 调 `auto_apply_on_startup` 幂等补写缺失文件。
//! - 仅管理员可调（web 路由层鉴权）。

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    time::Duration,
};

const SCHEMA_VERSION: u32 = 1;
const KEY_AGENT_CONFIG: &str = "agent_config";
const KEY_IMPORTED_AT: &str = "agent_config_imported_at";
const KEY_AUTO_APPLY: &str = "agent_config_auto_apply";

// ===================== Bundle schema (extensible, additive) =====================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentConfigBundle {
    pub schema_version: u32,
    pub exported_at: Option<i64>,
    pub exported_by: Option<String>,
    pub source: Option<String>,
    pub codex: CodexConfig,
    pub claude: ClaudeConfig,
    pub cursor: CursorConfig,
    pub opencode: OpenCodeConfig,
    /// 通用 home-relative 文件（逃生口，支持未来/未知 CLI，如 opencode.json）。
    /// Value 为 String 时按原始文本写；否则按 JSON 美化写。
    pub files: serde_json::Map<String, serde_json::Value>,
    /// 成员级覆盖：{ adapter, displayName?, memberId?, model?, apiKey?, executable? }
    pub agents: Vec<AgentProfilePatch>,
    /// 导入时希望尝试自动安装的 CLI（codex / claude / opencode / dsh / cursor）。
    #[serde(default)]
    pub auto_install: Vec<String>,
}

impl Default for AgentConfigBundle {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            exported_at: None,
            exported_by: None,
            source: Some("ohmyworkpanel".into()),
            codex: CodexConfig::default(),
            claude: ClaudeConfig::default(),
            cursor: CursorConfig::default(),
            opencode: OpenCodeConfig::default(),
            files: serde_json::Map::new(),
            agents: Vec::new(),
            auto_install: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodexConfig {
    pub enabled: bool,
    /// 默认 `http://127.0.0.1:18888/v1`（面板内嵌 shim）。
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub auth_mode: Option<String>,
}
impl Default for CodexConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: Some(crate::adapters::codex::base_url()),
            model: Some(crate::adapters::codex::DEFAULT_MODEL.into()),
            api_key: None,
            auth_mode: Some("apikey".into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClaudeConfig {
    pub enabled: bool,
    pub base_url: Option<String>,
    pub auth_token: Option<String>,
    pub model: Option<String>,
}
impl Default for ClaudeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: None,
            auth_token: None,
            model: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CursorConfig {
    pub enabled: bool,
    pub executable: Option<String>,
    pub model: Option<String>,
    /// 原样合并到 `~/.cursor/cli-config.json`（可选）。
    pub cli_config: Option<serde_json::Value>,
    /// 原样写入 `~/.cursor/mcp.json`（可选）。
    pub mcp: Option<serde_json::Value>,
}
impl Default for CursorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            executable: None,
            model: None,
            cli_config: None,
            mcp: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OpenCodeConfig {
    pub enabled: bool,
    pub model: Option<String>,
    pub api_key: Option<String>,
}
impl Default for OpenCodeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model: None,
            api_key: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentProfilePatch {
    pub adapter: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
}
impl Default for AgentProfilePatch {
    fn default() -> Self {
        Self {
            adapter: String::new(),
            display_name: None,
            member_id: None,
            model: None,
            api_key: None,
            executable: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProfileRow {
    pub member_id: String,
    pub group_id: String,
    pub display_name: String,
    pub adapter: String,
    pub model: Option<String>,
    pub executable_path: Option<String>,
    pub api_key_set: bool,
    pub system_locked: bool,
}

// ===================== Report / status =====================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportStep {
    pub name: String,
    pub status: String, // ok | warn | err
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub ok: bool,
    pub steps: Vec<ImportStep>,
    pub installed: Vec<String>,
    pub missing: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliPresence {
    pub cli: String,
    pub present: bool,
    pub path: Option<String>,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEnvStatus {
    pub node_path: Option<String>,
    pub shim_up: bool,
    pub shim_port: u16,
    pub clis: Vec<CliPresence>,
    pub codex_key_set: bool,
    pub claude_settings_present: bool,
    pub cursor_config_present: bool,
    pub bundle_imported_at: Option<i64>,
    pub auto_apply: bool,
    /// 脱敏后的有效配置（供前端展示）。
    pub effective: serde_json::Value,
}

// ===================== paths =====================

pub fn home_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\Users\\default"))
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/root"))
    }
}

pub fn codex_home() -> PathBuf {
    std::env::var("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".codex"))
}

fn file_exists(p: &Path) -> bool {
    std::fs::metadata(p).map(|m| m.is_file()).unwrap_or(false)
}

/// Copy existing file to `<path>.<ts>.bak` before overwriting. Returns the backup path.
fn backup_file(p: &Path) -> std::io::Result<Option<PathBuf>> {
    if !file_exists(p) {
        return Ok(None);
    }
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let bak = PathBuf::from(format!("{}.{}.bak", p.display(), stamp));
    std::fs::copy(p, &bak)?;
    Ok(Some(bak))
}

/// 安全地把 home 相对路径拼到 home 下：拒绝绝对路径、`..`、空。
fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = rel.trim().trim_start_matches("./");
    if rel.is_empty() || rel.starts_with('/') || rel.starts_with('\\') {
        return Err(format!("非法路径：{rel}"));
    }
    let mut out = root.to_path_buf();
    for part in rel.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(format!("非法路径（禁止 ..）：{rel}"));
        }
        out.push(part);
    }
    Ok(out)
}

/// 写文件前可选备份。返回 true 表示已写入；false 表示因 !overwrite 且已存在而跳过。
fn write_with_backup(path: &Path, content: &[u8], overwrite: bool) -> std::io::Result<bool> {
    if file_exists(path) && !overwrite {
        return Ok(false);
    }
    let _ = backup_file(path)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::File::create(path)?;
    f.write_all(content)?;
    Ok(true)
}

// ===================== redaction =====================

/// 就地脱敏：把密钥字段置空（导出不含密钥的包，导入时不会误写占位串）。
pub fn redact_bundle(mut bundle: AgentConfigBundle) -> AgentConfigBundle {
    bundle.codex.api_key = None;
    bundle.claude.auth_token = None;
    bundle.opencode.api_key = None;
    for a in bundle.agents.iter_mut() {
        a.api_key = None;
    }
    bundle
}

fn redact_value(v: &str) -> String {
    if v.trim().is_empty() {
        return String::new();
    }
    let v = v.trim();
    if v.len() <= 8 {
        return "****".into();
    }
    format!("{}******{}", &v[..4], &v[v.len() - 4..])
}

// ===================== export =====================

pub fn build_bundle(db_path: &Path, include_secrets: bool) -> Result<AgentConfigBundle, String> {
    let home = home_dir();
    let mut bundle = AgentConfigBundle::default();

    // Codex：key 来自 auth.json / env；base_url / model 跟随当前适配器默认（可被成员覆盖）。
    let codex_key = codex_resolved_key(None);
    bundle.codex.enabled = true;
    bundle.codex.base_url = Some(crate::adapters::codex::base_url());
    bundle.codex.model = Some(crate::adapters::codex::DEFAULT_MODEL.into());
    if let Some(k) = codex_key {
        bundle.codex.api_key = Some(k);
    }

    // Claude
    if let Some(cfg) = read_json_file(&home.join(".claude").join("settings.json")) {
        let env = cfg.get("env").and_then(|e| e.as_object());
        if let Some(env) = env {
            bundle.claude.enabled = true;
            if let Some(u) = env.get("ANTHROPIC_BASE_URL").and_then(|v| v.as_str()) {
                bundle.claude.base_url = Some(u.to_string());
            }
            if let Some(t) = env.get("ANTHROPIC_AUTH_TOKEN").and_then(|v| v.as_str()) {
                bundle.claude.auth_token = Some(t.to_string());
            }
        }
    }

    // Cursor
    let cursor_cli = cursor_executable_found();
    if cursor_cli.is_some() {
        bundle.cursor.enabled = true;
    }
    bundle.cursor.executable = Some("agent".into());
    if let Some(cfg) = read_json_file(&home.join(".cursor").join("cli-config.json")) {
        bundle.cursor.cli_config = Some(cfg);
    }
    if let Some(cfg) = read_json_file(&home.join(".cursor").join("mcp.json")) {
        bundle.cursor.mcp = Some(cfg);
    }

    // 成员映射（含各自 model / executable / codex key）
    let conn = crate::db::open_db(db_path)?;
    for row in crate::db::list_agent_profiles(&conn)? {
        if row.system_locked {
            continue;
        }
        let mut patch = AgentProfilePatch {
            adapter: row.adapter.clone(),
            display_name: Some(row.display_name.clone()),
            member_id: Some(row.member_id.clone()),
            ..Default::default()
        };
        if let Some(model) = row.model.clone() {
            patch.model = Some(model);
        }
        if let Some(exe) = row.executable_path.clone() {
            patch.executable = Some(exe);
        }
        if row.adapter == "codex" && include_secrets {
            if let Ok(Some(k)) = crate::db::get_agent_api_key(&conn, &row.member_id) {
                if !k.trim().is_empty() {
                    patch.api_key = Some(k);
                }
            }
        }
        bundle.agents.push(patch);
    }

    if !include_secrets {
        bundle = redact_bundle(bundle);
    } else {
        // 派生 codex 段若成员已有 key 且 auth 文件缺失，则带上
        if bundle.codex.api_key.is_none() {
            let conn2 = crate::db::open_db(db_path).ok();
            if let Some(conn2) = conn2 {
                if let Ok(rows) = crate::db::list_agent_profiles(&conn2) {
                    if let Some(first_codex) = rows.iter().find(|r| r.adapter == "codex") {
                        if let Ok(Some(k)) = crate::db::get_agent_api_key(&conn2, &first_codex.member_id)
                        {
                            if !k.trim().is_empty() {
                                bundle.codex.api_key = Some(k);
                            }
                        }
                    }
                }
            }
        }
    }
    bundle.source = Some("ohmyworkpanel/export".into());
    Ok(bundle)
}

// ===================== import / apply =====================

pub async fn import(
    db_path: &Path,
    mut bundle: AgentConfigBundle,
    auto_install_flag: Option<bool>,
    overwrite_flag: Option<bool>,
) -> ImportReport {
    let auto_install = auto_install_flag.unwrap_or(true);
    let overwrite = overwrite_flag.unwrap_or(true);
    let mut steps = Vec::new();
    let mut warnings = Vec::new();

    if bundle.schema_version == 0 {
        bundle.schema_version = SCHEMA_VERSION;
    }
    if bundle.schema_version > SCHEMA_VERSION {
        return ImportReport {
            ok: false,
            steps: vec![ImportStep {
                name: "配置包版本".into(),
                status: "err".into(),
                detail: format!(
                    "配置包 schemaVersion={} 高于本版本支持 {}，请升级面板后再导入。",
                    bundle.schema_version, SCHEMA_VERSION
                ),
            }],
            installed: Vec::new(),
            missing: Vec::new(),
            warnings,
        };
    }

    // 1) node 前置（shim 与 npm 安装都需要）
    match crate::adapters::find_executable_on_path("node") {
        Some(p) => steps.push(step_ok("Node.js", &format!("已找到：{p}"))),
        None => {
            steps.push(step_err("Node.js", "未找到 node。Codex shim 与 CLI 自动安装都依赖 node。"));
            warnings.push("未检测到 Node.js：无法自动安装 CLI，请先安装 Node 或在服务器上手动配置。".into());
        }
    }

    // 2) 写 home 配置（尽力而为，逐文件记录）
    for (name, res) in [
        ("Codex 配置".to_string(), apply_codex(&bundle, overwrite)),
        ("Claude Code 配置".to_string(), apply_claude(&bundle, overwrite)),
        ("Cursor 配置".to_string(), apply_cursor(&bundle, overwrite)),
        ("通用配置(files)".to_string(), apply_files(&bundle, overwrite)),
    ] {
        match res {
            Ok(detail) if detail.trim().is_empty() => {}
            Ok(detail) => steps.push(step_ok(&name, &detail)),
            Err(e) => {
                steps.push(step_err(&name, &e));
                warnings.push(format!("{name}：{e}"));
            }
        }
    }

    // 3) 同步成员（agent_profiles）
    match crate::db::open_db(db_path) {
        Ok(conn) => match provision_agents(&conn, &bundle) {
            Ok(detail) if !detail.is_empty() => {
                steps.push(step_ok("同步 Agent 成员", &detail));
            }
            _ => {}
        },
        Err(e) => warnings.push(format!("打开数据库失败，跳过成员同步：{e}")),
    }

    // 4) 持久化 + 启动自动重放
    let persisted = bundle.json_persisted();
    match crate::db::open_db(db_path) {
        Ok(conn) => {
            let _ = crate::db::set_setting_str(&conn, KEY_AGENT_CONFIG, &persisted);
            let _ = crate::db::set_setting_str(
                &conn,
                KEY_IMPORTED_AT,
                &chrono::Utc::now().timestamp_millis().to_string(),
            );
            let _ = crate::db::set_setting_str(&conn, KEY_AUTO_APPLY, "1");
            steps.push(step_ok(
                "持久化配置",
                "已保存，后续每次启动将自动补写缺失配置（幂等）。",
            ));
        }
        Err(e) => warnings.push(format!("持久化配置失败：{e}")),
    }

    // 5) 自动安装缺失 CLI
    let mut installed = Vec::new();
    let mut missing = Vec::new();
    if auto_install {
        for cli in bundle
            .auto_install
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
        {
            if cli_present(&cli) {
                continue;
            }
            let (ok, detail) = run_install(&cli).await;
            if ok && cli_present(&cli) {
                installed.push(cli.clone());
                steps.push(step_ok(&format!("安装 {cli}"), &detail));
            } else {
                steps.push(step_warn(
                    &format!("安装 {cli}"),
                    &format!("未成功：{detail}（可手动安装后重试）"),
                ));
                warnings.push(format!("{cli} 未安装成功：{detail}"));
                missing.push(cli.clone());
            }
        }
    }
    for cli in bundle
        .auto_install
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && !cli_present(s))
    {
        if !missing.contains(&cli) && !installed.contains(&cli) {
            missing.push(cli);
        }
    }
    if auto_install {
        steps.push(step_ok(
            "环境回检",
            &if missing.is_empty() {
                "所有勾选 CLI 就绪。".into()
            } else {
                format!("仍缺：{}；可稍后再点「自动安装」或手动处理。", missing.join(", "))
            },
        ));
    }

    let ok = steps.iter().all(|s| s.status != "err");
    ImportReport {
        ok,
        steps,
        installed,
        missing: if missing.is_empty() { Vec::new() } else { missing },
        warnings,
    }
}

impl AgentConfigBundle {
    /// 持久化时的形态：导出时间戳 + 成员 key 冗余填充（供启动重放直接用）。
    fn json_persisted(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }
}

fn step_ok(name: &str, detail: &str) -> ImportStep {
    ImportStep {
        name: name.into(),
        status: "ok".into(),
        detail: detail.into(),
    }
}
fn step_warn(name: &str, detail: &str) -> ImportStep {
    ImportStep {
        name: name.into(),
        status: "warn".into(),
        detail: detail.into(),
    }
}
fn step_err(name: &str, detail: &str) -> ImportStep {
    ImportStep {
        name: name.into(),
        status: "err".into(),
        detail: detail.into(),
    }
}

fn apply_codex(bundle: &AgentConfigBundle, overwrite: bool) -> Result<String, String> {
    if !bundle.codex.enabled || bundle.codex.api_key.as_deref().map(str::trim).unwrap_or("").is_empty()
    {
        return Ok(String::new());
    }
    let dir = codex_home();
    let mut detail = String::new();
    // credential 文件：有 key 才写（不会清空已有 key）
    let auth = json!({
        "OPENAI_API_KEY": bundle.codex.api_key.as_deref().unwrap_or("").trim(),
        "auth_mode": bundle.codex.auth_mode.as_deref().filter(|s| !s.is_empty()).unwrap_or("apikey"),
    });
    let auth_path = dir.join("auth.json");
    let auth_json = serde_json::to_string_pretty(&auth).map_err(|e| e.to_string())?;
    let wrote = write_with_backup(&auth_path, auth_json.as_bytes(), overwrite)
        .map_err(|e| format!("写 ~/.codex/auth.json 失败：{e}"))?;
    if wrote {
        detail.push_str("写入 ~/.codex/auth.json；");
    } else {
        detail.push_str("~/.codex/auth.json 已存在（保留）；");
    }
    // config.toml：仅缺省时写入最小 provider（不覆盖 vibecoding 已有配置）
    let config_path = dir.join("config.toml");
    if !file_exists(&config_path) {
        let base = bundle
            .codex
            .base_url
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| crate::adapters::codex::base_url());
        let model = bundle
            .codex
            .model
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(crate::adapters::codex::DEFAULT_MODEL);
        let toml = format!(
            "model_provider = \"deepseek\"\nmodel = \"{model}\"\napproval_policy = \"never\"\n\n[model_providers.deepseek]\nname = \"deepseek\"\nbase_url = \"{base}\"\nenv_key = \"OPENAI_API_KEY\"\nwire_api = \"responses\"\n"
        );
        write_with_backup(&config_path, toml.as_bytes(), overwrite)
            .map_err(|e| format!("写 ~/.codex/config.toml 失败：{e}"))?;
        detail.push_str("新建 provider 配置(config.toml)；");
    } else {
        detail.push_str("config.toml 已存在（保留）；");
    }
    Ok(detail)
}

fn apply_claude(bundle: &AgentConfigBundle, overwrite: bool) -> Result<String, String> {
    if !bundle.claude.enabled {
        return Ok(String::new());
    }
    let has_base = bundle
        .claude
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_some();
    let has_token = bundle
        .claude
        .auth_token
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_some();
    if !has_base && !has_token {
        return Ok(String::new());
    }
    let path = home_dir().join(".claude").join("settings.json");
    let mut cfg = read_json_file(&path).unwrap_or_else(|| json!({}));
    // settings.json 顶层也可能是 env 直放，规范做法是 env 对象；我们规范化到 env。
    let env_obj = cfg.get_mut("env");
    let mut env = match env_obj {
        Some(serde_json::Value::Object(m)) => m.clone(),
        _ => serde_json::Map::new(),
    };
    if has_base {
        env.insert(
            "ANTHROPIC_BASE_URL".into(),
            json!(bundle.claude.base_url.as_deref().unwrap_or("").trim()),
        );
    }
    if has_token {
        env.insert(
            "ANTHROPIC_AUTH_TOKEN".into(),
            json!(bundle.claude.auth_token.as_deref().unwrap_or("").trim()),
        );
    }
    cfg.as_object_mut()
        .map(|o| {
            o.insert("env".into(), json!(env));
        })
        .ok_or_else(|| "settings.json 结构异常".to_string())?;
    let wrote = write_with_backup(
        &path,
        serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?.as_bytes(),
        overwrite,
    )
    .map_err(|e| format!("写 ~/.claude/settings.json 失败：{e}"))?;
    Ok(if wrote {
        "写入 ~/.claude/settings.json（env 合并）".into()
    } else {
        "~/.claude/settings.json 已存在（保留，未覆盖；设置 overwrite 为 true 可覆盖）".into()
    })
}

fn deep_merge(base: &mut serde_json::Value, patch: &serde_json::Value) {
    match (base, patch) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            for (k, v) in b {
                if let Some(existing) = a.get_mut(k) {
                    if existing.is_object() && v.is_object() {
                        deep_merge(existing, v);
                    } else {
                        a.insert(k.clone(), v.clone());
                    }
                } else {
                    a.insert(k.clone(), v.clone());
                }
            }
        }
        (a, b) => *a = b.clone(),
    }
}

fn apply_cursor(bundle: &AgentConfigBundle, overwrite: bool) -> Result<String, String> {
    if !bundle.cursor.enabled {
        return Ok(String::new());
    }
    let dir = home_dir().join(".cursor");
    let mut detail = String::new();
    if let Some(patch) = &bundle.cursor.cli_config {
        let path = dir.join("cli-config.json");
        let mut cfg = read_json_file(&path).unwrap_or_else(|| json!({}));
        deep_merge(&mut cfg, patch);
        let wrote = write_with_backup(
            &path,
            serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?.as_bytes(),
            overwrite,
        )
        .map_err(|e| format!("写 ~/.cursor/cli-config.json 失败：{e}"))?;
        detail.push_str(if wrote {
            "写入 ~/.cursor/cli-config.json；"
        } else {
            "~/.cursor/cli-config.json 已存在（保留）；"
        });
    }
    if let Some(mcp) = &bundle.cursor.mcp {
        let path = dir.join("mcp.json");
        let wrote = write_with_backup(
            &path,
            serde_json::to_string_pretty(mcp).map_err(|e| e.to_string())?.as_bytes(),
            overwrite,
        )
        .map_err(|e| format!("写 ~/.cursor/mcp.json 失败：{e}"))?;
        detail.push_str(if wrote {
            "写入 ~/.cursor/mcp.json；"
        } else {
            "~/.cursor/mcp.json 已存在（保留）；"
        });
    }
    Ok(detail)
}

fn apply_files(bundle: &AgentConfigBundle, overwrite: bool) -> Result<String, String> {
    if bundle.files.is_empty() {
        return Ok(String::new());
    }
    let home = home_dir();
    let mut done = Vec::new();
    for (rel, value) in &bundle.files {
        let path = safe_join(&home, rel)?;
        let content = match value {
            serde_json::Value::String(s) => s.clone().into_bytes(),
            _ => serde_json::to_string_pretty(value)
                .map_err(|e| e.to_string())?
                .into_bytes(),
        };
        let wrote = write_with_backup(&path, &content, overwrite)
            .map_err(|e| format!("写 {rel} 失败：{e}"))?;
        done.push(if wrote {
            format!("~/{rel}")
        } else {
            format!("~/{rel}（已存在，保留）")
        });
    }
    Ok(format!("写入 {}", done.join("、")))
}

/// 把 bundle 应用到既有 agent 成员（只更新，不新建；跳过 system_locked）。
fn provision_agents(conn: &rusqlite::Connection, bundle: &AgentConfigBundle) -> Result<String, String> {
    let rows = crate::db::list_agent_profiles(conn)?;
    if rows.is_empty() {
        return Ok(String::new());
    }
    let mut detail = Vec::new();
    let mut changed = 0usize;

    // 显式 patch（精确到成员/displayName）
    for patch in &bundle.agents {
        let target = rows
            .iter()
            .find(|r| {
                !r.system_locked
                    && r.adapter == patch.adapter
                    && patch
                        .member_id
                        .as_deref()
                        .map(|id| id == r.member_id)
                        .unwrap_or(true)
            })
            .or_else(|| {
                rows.iter().find(|r| {
                    !r.system_locked
                        && r.adapter == patch.adapter
                        && patch
                            .display_name
                            .as_deref()
                            .map(|d| d.eq_ignore_ascii_case(&r.display_name))
                            .unwrap_or(true)
                })
            });
        let Some(row) = target else { continue };
        apply_patch(conn, row, patch);
        changed += 1;
        detail.push(row.display_name.clone());
    }

    // 顶层 enabled 段 → 按 adapter 应用到所有匹配成员（默认值兜底；只有 enabled 段生效）
    let sections: [(&str, bool, Option<String>, Option<String>, Option<String>); 4] = [
        (
            "codex",
            bundle.codex.enabled,
            bundle.codex.model.clone(),
            bundle.codex.api_key.clone(),
            None,
        ),
        (
            "claude-code",
            bundle.claude.enabled,
            bundle.claude.model.clone(),
            bundle.claude.auth_token.clone(),
            None,
        ),
        (
            "cursor",
            bundle.cursor.enabled,
            bundle.cursor.model.clone(),
            None,
            bundle.cursor.executable.clone(),
        ),
        (
            "opencode",
            bundle.opencode.enabled,
            bundle.opencode.model.clone(),
            bundle.opencode.api_key.clone(),
            None,
        ),
    ];
    for (adapter, enabled, model, api_key, executable) in sections {
        if !enabled {
            continue;
        }
        for row in rows.iter().filter(|r| !r.system_locked && r.adapter == adapter) {
            let p = AgentProfilePatch {
                adapter: adapter.to_string(),
                model: model
                    .clone()
                    .filter(|m| !m.trim().is_empty() && m != "default"),
                api_key: api_key.clone().filter(|k| !k.trim().is_empty()),
                executable: executable.clone().filter(|e| !e.trim().is_empty()),
                ..Default::default()
            };
            if p.model.is_none() && p.api_key.is_none() && p.executable.is_none() {
                continue;
            }
            apply_patch(conn, row, &p);
            changed += 1;
            if !detail.contains(&row.display_name) {
                detail.push(row.display_name.clone());
            }
        }
    }

    if changed == 0 {
        return Ok(String::new());
    }
    Ok(format!("已更新 {} 个成员：{}", changed, detail.join("、")))
}

fn apply_patch(
    conn: &rusqlite::Connection,
    row: &AgentProfileRow,
    patch: &AgentProfilePatch,
) {
    if let Some(m) = patch
        .model
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "default")
    {
        let _ = crate::db::set_member_model(conn, &row.member_id, Some(m));
    }
    if let Some(k) = patch.api_key.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let _ = crate::db::set_member_api_key(conn, &row.member_id, Some(k));
    }
    if let Some(e) = patch.executable.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let _ = crate::db::set_member_executable(conn, &row.member_id, Some(e));
    }
}

// ===================== auto install =====================

pub(crate) struct InstallSpec {
    name: &'static str,
    desc: String,
    command: Vec<String>,
}

fn npm_install_cmd(pkg: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        vec!["cmd".into(), "/c".into(), "npm".into(), "install".into(), "-g".into(), pkg.into()]
    }
    #[cfg(not(windows))]
    {
        vec!["npm".into(), "install".into(), "-g".into(), pkg.into()]
    }
}

fn cursor_install_cmd() -> Vec<String> {
    #[cfg(windows)]
    {
        vec![
            "powershell".into(),
            "-NoProfile".into(),
            "-ExecutionPolicy".into(),
            "Bypass".into(),
            "-Command".into(),
            "irm 'https://cursor.com/install?win32=true' | iex".into(),
        ]
    }
    #[cfg(not(windows))]
    {
        vec![
            "bash".into(),
            "-c".into(),
            "set -euo pipefail; curl -fsSL https://cursor.com/install | bash".into(),
        ]
    }
}

/// CLI 的官方安装命令（best-effort；可被 users 在文档中手动替换）。
pub(crate) fn install_spec(cli: &str) -> Option<InstallSpec> {
    let spec = match cli.trim() {
        "codex" => InstallSpec {
            name: "codex",
            desc: "OpenAI Codex CLI".into(),
            command: npm_install_cmd("@openai/codex"),
        },
        "claude" | "claude-code" => InstallSpec {
            name: "claude",
            desc: "Claude Code CLI".into(),
            command: npm_install_cmd("@anthropic-ai/claude-code"),
        },
        "opencode" => InstallSpec {
            name: "opencode",
            desc: "OpenCode CLI".into(),
            command: npm_install_cmd("opencode-ai"),
        },
        "dsh" => InstallSpec {
            name: "dsh",
            desc: "DeepSeek Harness CLI".into(),
            command: npm_install_cmd("@deepseek-ai/dsh"),
        },
        "cursor" => InstallSpec {
            name: "cursor",
            desc: "Cursor CLI（agent）".into(),
            command: cursor_install_cmd(),
        },
        _ => return None,
    };
    Some(spec)
}

pub fn cli_present(cli: &str) -> bool {
    match cli.trim() {
        "cursor" => cursor_executable_found().is_some(),
        _ => crate::adapters::find_executable_on_path(cli.trim()).is_some(),
    }
}

pub fn cursor_executable_found() -> Option<String> {
    crate::adapters::AdapterKind::Cursor
        .candidate_executables()
        .iter()
        .find_map(|name| crate::adapters::find_executable_on_path(name))
}

pub async fn run_install(cli: &str) -> (bool, String) {
    let Some(spec) = install_spec(cli) else {
        return (false, format!("未知 CLI：{cli}"));
    };
    if cli_present(cli) {
        return (true, format!("{} 已就绪，无需安装。", spec.name));
    }
    run_command(&spec.command, 180).await
}

pub async fn run_command(command: &[String], timeout_secs: u64) -> (bool, String) {
    if command.is_empty() {
        return (false, "空命令".into());
    }
    use tokio::process::Command as TCommand;
    let mut child = match TCommand::new(&command[0])
        .args(&command[1..])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return (false, format!("无法启动 {}：{e}", command[0])),
    };
    let out = child.stdout.take();
    let err = child.stderr.take();
    let read_task = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut so = String::new();
        let mut se = String::new();
        if let Some(mut o) = out {
            let _ = o.read_to_string(&mut so).await;
        }
        if let Some(mut e) = err {
            let _ = e.read_to_string(&mut se).await;
        }
        (so, se)
    });
    let waited = tokio::time::timeout(Duration::from_secs(timeout_secs), child.wait()).await;
    let (ok, tail) = match waited {
        Ok(Ok(status)) => {
            let (so, se) = read_task.await.unwrap_or_default();
            let text = if !se.trim().is_empty() { se } else { so };
            (
                status.success(),
                text.trim().chars().take(800).collect::<String>(),
            )
        }
        Ok(Err(e)) => {
            let _ = child.kill().await;
            (false, format!("进程错误:{e}"))
        }
        Err(_) => {
            let _ = child.kill().await;
            (false, format!("{timeout_secs}s 超时"))
        }
    };
    let detail = if tail.is_empty() {
        if ok {
            format!("{} 执行成功。", command.join(" "))
        } else {
            format!("{} 执行失败（无输出）。", command[0])
        }
    } else {
        tail
    };
    (ok, detail)
}

// ===================== status =====================

pub fn port_open(port: u16) -> bool {
    TcpStream::connect(("127.0.0.1", port))
        .map(|s| {
            let _ = s.shutdown(std::net::Shutdown::Both);
            true
        })
        .unwrap_or(false)
}

fn codex_resolved_key(explicit: Option<&str>) -> Option<String> {
    crate::adapters::codex::resolve_api_key(explicit)
}

pub fn status(db_path: &Path) -> AgentEnvStatus {
    let home = home_dir();
    let node_path = crate::adapters::find_executable_on_path("node");
    let shim_port = std::env::var("OHMYWORKPANEL_CODEX_PROXY_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(18888);

    let clis = vec![
        CliPresence {
            cli: "codex".into(),
            present: crate::adapters::find_executable_on_path("codex").is_some(),
            path: crate::adapters::find_executable_on_path("codex"),
            label: "Codex CLI（codex）".into(),
        },
        CliPresence {
            cli: "cursor".into(),
            present: cursor_executable_found().is_some(),
            path: cursor_executable_found(),
            label: "Cursor CLI（agent)）".into(),
        },
        CliPresence {
            cli: "claude".into(),
            present: crate::adapters::find_executable_on_path("claude").is_some(),
            path: crate::adapters::find_executable_on_path("claude"),
            label: "Claude Code CLI（claude）".into(),
        },
        CliPresence {
            cli: "opencode".into(),
            present: crate::adapters::find_executable_on_path("opencode").is_some(),
            path: crate::adapters::find_executable_on_path("opencode"),
            label: "OpenCode CLI（opencode）".into(),
        },
        CliPresence {
            cli: "dsh".into(),
            present: crate::adapters::find_executable_on_path("dsh").is_some(),
            path: crate::adapters::find_executable_on_path("dsh"),
            label: "DeepSeek Harness（dsh）".into(),
        },
    ];

    let (bundle_imported_at, auto_apply, persisted_bundle) =
        if let Ok(conn) = crate::db::open_db(db_path) {
            let imported = crate::db::get_setting_str(&conn, KEY_IMPORTED_AT)
                .ok()
                .flatten()
                .and_then(|s| s.parse().ok());
            let aa = crate::db::get_setting_str(&conn, KEY_AUTO_APPLY)
                .ok()
                .flatten()
                .map(|s| s != "0")
                .unwrap_or(true);
            let bundle = crate::db::get_setting_str(&conn, KEY_AGENT_CONFIG)
                .ok()
                .flatten();
            (imported, aa, bundle)
        } else {
            (None, true, None)
        };

    let codex_key_set = codex_resolved_key(None).is_some();

    let effective = {
        let mut bundle = build_bundle(db_path, false).unwrap_or_default();
        bundle.schema_version = SCHEMA_VERSION;
        let exported = persisted_bundle
            .as_deref()
            .and_then(|s| serde_json::from_str::<AgentConfigBundle>(s).ok());
        if let Some(p) = &exported {
            bundle.codex.enabled = bundle.codex.enabled || p.codex.enabled;
            bundle.claude.enabled = p.claude.enabled;
            bundle.cursor.enabled = bundle.cursor.enabled || p.cursor.enabled;
            bundle.opencode.enabled = bundle.opencode.enabled || p.opencode.enabled;
            bundle.claude = redact_claude(p.claude.clone());
        }
        json!(bundle)
    };

    AgentEnvStatus {
        node_path,
        shim_up: port_open(shim_port),
        shim_port,
        clis,
        codex_key_set,
        claude_settings_present: file_exists(&home.join(".claude").join("settings.json")),
        cursor_config_present: file_exists(&home.join(".cursor").join("cli-config.json")),
        bundle_imported_at,
        auto_apply,
        effective,
    }
}

fn redact_claude(c: ClaudeConfig) -> ClaudeConfig {
    ClaudeConfig {
        enabled: c.enabled,
        base_url: c.base_url,
        auth_token: c.auth_token.map(|t| redact_value(&t)),
        model: c.model,
    }
}

/// 启动时幂等重放：读持久化配置，缺失文件补写（不覆盖已有、不重装 CLI）。
pub fn auto_apply_on_startup(db_path: &Path) -> Result<(), String> {
    let conn = crate::db::open_db(db_path)?;
    let Some(raw) = crate::db::get_setting_str(&conn, KEY_AGENT_CONFIG)? else {
        return Ok(());
    };
    let aa = crate::db::get_setting_str(&conn, KEY_AUTO_APPLY)
        .ok()
        .flatten()
        .map(|s| s != "0")
        .unwrap_or(true);
    if !aa {
        return Ok(());
    }
    let bundle: AgentConfigBundle = serde_json::from_str(&raw).map_err(|e| {
        format!("agent_config 反序列化失败，跳过启动重放：{e}")
    })?;
    let _ = apply_codex(&bundle, false);
    let _ = apply_claude(&bundle, false);
    let _ = apply_cursor(&bundle, false);
    let _ = apply_files(&bundle, false);
    Ok(())
}

// ===================== helpers =====================

fn read_json_file(path: &Path) -> Option<serde_json::Value> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    serde_json::from_str(&s).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_bundle() -> AgentConfigBundle {
        let mut b = AgentConfigBundle::default();
        b.codex.enabled = true;
        b.codex.api_key = Some("sk-test-key-123456789012345678901234567890".into());
        b.codex.model = Some("deepseek-v4-flash".into());
        b.auto_install = Vec::new();
        b
    }

    #[test]
    fn schema_uses_camel_case_and_roundtrips() {
        let b = test_bundle();
        let s = serde_json::to_value(&b).unwrap();
        assert_eq!(s["schemaVersion"].as_u64(), Some(1));
        assert!(s["codex"]["baseUrl"].is_string());
        assert_eq!(s["codex"]["apiKey"], json!("sk-test-key-123456789012345678901234567890"));
        assert!(s.get("autoInstall").is_some());
        let json = serde_json::to_string(&b).unwrap();
        let back: AgentConfigBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(back.codex.model.as_deref(), Some("deepseek-v4-flash"));
    }

    #[test]
    fn missing_fields_default() {
        let b: AgentConfigBundle = serde_json::from_str(r#"{"codex":{"enabled":true}}"#).unwrap();
        assert_eq!(b.schema_version, SCHEMA_VERSION);
        assert!(b.auto_install.is_empty());
        assert!(b.agents.is_empty());
    }

    #[test]
    fn safe_join_rejects_traversal() {
        let root = Path::new("C:\\Users\\test");
        assert!(safe_join(root, "../etc/passwd").is_err());
        assert!(safe_join(root, "/etc/passwd").is_err());
        assert!(safe_join(root, "..\\evil").is_err());
        assert!(safe_join(root, "").is_err());
        assert!(safe_join(root, ".codex\\..\\evil").is_err());
        let ok = safe_join(root, ".codex/auth.json").unwrap();
        assert_eq!(ok, root.join(".codex").join("auth.json"));
    }

    #[test]
    fn redact_clears_keys_for_safe_reimport() {
        let mut b = test_bundle();
        b.agents.push(AgentProfilePatch {
            adapter: "codex".into(),
            api_key: Some("sk-x".into()),
            ..Default::default()
        });
        let r = redact_bundle(b);
        assert!(r.codex.api_key.is_none());
        assert!(r.claude.auth_token.is_none());
        assert!(r.agents[0].api_key.is_none());
        assert_eq!(r.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn v2_cursor_release_bundle_deserializes_without_secrets() {
        let raw = include_str!("../../../docs/releases/v2.0.0/cursor-agent.bundle.json");
        assert!(
            !raw.to_ascii_lowercase().contains("authid"),
            "release bundle must not ship Cursor auth"
        );
        let b: AgentConfigBundle = serde_json::from_str(raw).expect("v2.0.0 cursor bundle");
        assert!(b.cursor.enabled);
        assert_eq!(b.cursor.executable.as_deref(), Some("agent"));
        assert_eq!(b.cursor.model.as_deref(), Some("grok-4.6"));
        assert!(b.codex.api_key.is_none());
        assert!(b.claude.auth_token.is_none());
        assert!(b.agents.iter().all(|a| a.api_key.is_none()));
        assert!(b.agents.iter().any(|a| a.adapter == "cursor"));
        assert!(b.auto_install.iter().any(|c| c == "cursor"));
    }

    #[test]
    fn provision_updates_seed_codex_member() {
        let dir = TempDir::new().unwrap();
        let dbp = dir.path().join("test.sqlite3");
        crate::db::init_db(&dbp).unwrap();
        let conn = crate::db::open_db(&dbp).unwrap();
        let mut b = test_bundle();
        b.codex.api_key = Some("sk-provision-1234567890".into());
        b.codex.model = Some("deepseek-v4-flash".into());
        let detail = provision_agents(&conn, &b).unwrap();
        assert!(detail.contains("Codex"), "detail={detail}");
        let key = crate::db::get_agent_api_key(&conn, "seed-member-codex").unwrap();
        assert_eq!(key.as_deref(), Some("sk-provision-1234567890"));
    }

    #[test]
    fn apply_codex_writes_auth_only_when_key_present() {
        let dir = TempDir::new().unwrap();
        let home = dir.path().to_path_buf();
        std::env::set_var("CODEX_HOME", home.join(".codex"));
        let mut b = test_bundle();
        b.codex.api_key = None;
        let r = apply_codex(&b, true).unwrap();
        assert!(r.trim().is_empty(), "no key -> no write, got {r}");
        b.codex.api_key = Some("sk-write-1234567890".into());
        let r = apply_codex(&b, true).unwrap();
        assert!(r.contains("auth.json"));
        let auth = std::fs::read_to_string(home.join(".codex").join("auth.json")).unwrap();
        assert!(auth.contains("sk-write-1234567890"));
        std::env::remove_var("CODEX_HOME");
    }

    #[test]
    fn cli_map_known() {
        assert!(install_spec("codex").is_some());
        assert!(install_spec("claude").is_some());
        assert!(install_spec("opencode").is_some());
        assert!(install_spec("cursor").is_some());
        assert!(install_spec("dsh").is_some());
        assert!(install_spec("unknown").is_none());
    }
}
