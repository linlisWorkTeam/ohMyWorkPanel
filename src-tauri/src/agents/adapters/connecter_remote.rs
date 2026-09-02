use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::db::{AppResult, ConnecterProviderProfile};

pub const ADAPTER_ID: &str = "connecter-remote";

#[derive(Debug, Clone)]
pub struct ValidatedProfileInput {
    pub base_url: String,
    pub bearer_token: String,
    pub group_ref: String,
    pub target_subject_id: String,
    pub env: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchSnapshot {
    pub dispatch_id: String,
    pub status: String,
    #[serde(default)]
    pub result: Option<DispatchResult>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchResult {
    #[serde(default)]
    pub content: Value,
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    body: String,
}

pub fn is_connecter_remote(adapter: &str) -> bool {
    adapter == ADAPTER_ID
}

pub fn validate_profile_input(
    base_url: Option<&str>,
    bearer_token: Option<&str>,
    group_ref: Option<&str>,
    target_subject_id: Option<&str>,
    env: Option<&str>,
) -> AppResult<ValidatedProfileInput> {
    let base_url = normalize_base_url(base_url.unwrap_or_default())?;
    let required = |value: Option<&str>, label: &str| -> AppResult<String> {
        let value = value.unwrap_or_default().trim();
        if value.is_empty() {
            return Err(format!("Connecter {label} 不能为空。"));
        }
        if value.chars().any(char::is_control) {
            return Err(format!("Connecter {label} 含非法控制字符。"));
        }
        Ok(value.to_string())
    };
    let bearer_token = required(bearer_token, "bearer")?;
    let group_ref = required(group_ref, "groupRef")?;
    let target_subject_id = required(target_subject_id, "targetSubjectId")?;
    let env = required(env, "env")?;
    if bearer_token.chars().any(char::is_whitespace) {
        return Err("Connecter bearer 不能包含空白字符。".into());
    }
    validate_group_ref(&group_ref)?;
    if target_subject_id.len() > 256 || target_subject_id.chars().any(char::is_whitespace) {
        return Err("Connecter targetSubjectId 格式无效。".into());
    }
    if env.len() > 64
        || !env
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err("Connecter env 格式无效。".into());
    }
    Ok(ValidatedProfileInput {
        base_url,
        bearer_token,
        group_ref,
        target_subject_id,
        env,
    })
}

fn validate_group_ref(value: &str) -> AppResult<()> {
    let mut parts = value.splitn(3, ':');
    let prefix = parts.next().unwrap_or_default();
    let site_id = parts.next().unwrap_or_default();
    let group_id = parts.next().unwrap_or_default();
    let site_valid = !site_id.is_empty()
        && site_id.len() <= 63
        && site_id
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && site_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if prefix != "wp"
        || !site_valid
        || group_id.is_empty()
        || group_id.len() > 768
        || group_id.chars().any(char::is_whitespace)
    {
        return Err("Connecter groupRef 必须为 wp:<site-id>:<group-id>。".into());
    }
    Ok(())
}

pub fn validate_stored_profile(profile: &ConnecterProviderProfile) -> AppResult<()> {
    validate_profile_input(
        Some(&profile.base_url),
        Some(&profile.bearer_token),
        Some(&profile.group_ref),
        Some(&profile.target_subject_id),
        Some(&profile.env),
    )?;
    Ok(())
}

pub fn normalize_base_url(input: &str) -> AppResult<String> {
    let value = input.trim().trim_end_matches('/');
    let rest = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
        .ok_or_else(|| "Connecter base URL 只允许 http:// 或 https://。".to_string())?;
    let authority = rest.split('/').next().unwrap_or_default();
    if rest.is_empty()
        || authority.is_empty()
        || authority.starts_with(':')
        || value.chars().any(|c| c.is_whitespace() || c.is_control())
        || authority.contains('@')
        || value.contains('#')
        || value.contains('?')
    {
        return Err("Connecter base URL 无效。".into());
    }
    Ok(value.to_string())
}

pub async fn create_dispatch(
    profile: &ConnecterProviderProfile,
    run_id: &str,
    group_id: &str,
    member_id: &str,
    group_name: &str,
    prompt: &str,
) -> AppResult<DispatchSnapshot> {
    validate_stored_profile(profile)?;
    let body = json!({
        "groupRef": profile.group_ref,
        "targetSubjectId": profile.target_subject_id,
        "env": profile.env,
        "groupName": group_name,
        "prompt": prompt,
        "writeBack": false,
        "context": {
            "workPanelRunId": run_id,
            "groupId": group_id,
            "memberId": member_id,
        }
    });
    let response = curl_request(
        profile,
        "POST",
        "/v2/dispatches",
        Some(("Idempotency-Key", run_id)),
        Some(&body),
    )
    .await?;
    if !matches!(response.status, 200 | 202) {
        return Err(http_error("创建 Connecter dispatch", &response));
    }
    parse_snapshot(&response.body)
}

pub async fn get_dispatch(
    profile: &ConnecterProviderProfile,
    dispatch_id: &str,
) -> AppResult<DispatchSnapshot> {
    let response = curl_request(
        profile,
        "GET",
        &format!("/v2/dispatches/{dispatch_id}"),
        None,
        None,
    )
    .await?;
    if response.status != 200 {
        return Err(http_error("读取 Connecter dispatch", &response));
    }
    parse_snapshot(&response.body)
}

pub async fn cancel_dispatch(
    profile: &ConnecterProviderProfile,
    dispatch_id: &str,
) -> AppResult<()> {
    let body = json!({ "reason": "cancelled by WorkPanel provider" });
    let response = curl_request(
        profile,
        "POST",
        &format!("/v2/dispatches/{dispatch_id}/cancel"),
        None,
        Some(&body),
    )
    .await?;
    if matches!(response.status, 200 | 202) {
        Ok(())
    } else {
        Err(http_error("取消 Connecter dispatch", &response))
    }
}

pub async fn poll_until_terminal(
    profile: &ConnecterProviderProfile,
    mut snapshot: DispatchSnapshot,
    timeout: Duration,
    poll_interval: Duration,
    token: &Arc<AtomicBool>,
) -> AppResult<String> {
    let started = Instant::now();
    loop {
        if token.load(Ordering::SeqCst) {
            let _ = cancel_dispatch(profile, &snapshot.dispatch_id).await;
            return Err("Connecter 远程任务已取消。".into());
        }
        match snapshot.status.as_str() {
            "completed" => return completed_text(&snapshot),
            "failed" | "dead" => {
                return Err(format!(
                    "Connecter 远程任务 {}：{}",
                    snapshot.status,
                    snapshot.error.as_deref().unwrap_or("未提供错误详情")
                ));
            }
            "cancelled" => return Err("Connecter 远程任务已取消。".into()),
            _ => {}
        }
        if started.elapsed() >= timeout {
            let _ = cancel_dispatch(profile, &snapshot.dispatch_id).await;
            return Err("Connecter 远程任务超时，已请求取消。".into());
        }
        tokio::time::sleep(poll_interval).await;
        snapshot = get_dispatch(profile, &snapshot.dispatch_id).await?;
    }
}

fn completed_text(snapshot: &DispatchSnapshot) -> AppResult<String> {
    let content = snapshot
        .result
        .as_ref()
        .map(|result| &result.content)
        .ok_or_else(|| "Connecter completed dispatch 缺少 result。".to_string())?;
    match content {
        Value::String(text) => Ok(text.clone()),
        Value::Object(map) => match map.get("text").and_then(Value::as_str) {
            Some(text) => Ok(text.to_string()),
            None => serde_json::to_string(content).map_err(|e| e.to_string()),
        },
        Value::Null => Err("Connecter completed dispatch 的 result.content 为空。".into()),
        _ => serde_json::to_string(content).map_err(|e| e.to_string()),
    }
}

fn parse_snapshot(body: &str) -> AppResult<DispatchSnapshot> {
    let snapshot: DispatchSnapshot = serde_json::from_str(body)
        .map_err(|e| format!("Connecter dispatch 响应不是有效 JSON：{e}"))?;
    if snapshot.dispatch_id.trim().is_empty()
        || snapshot.dispatch_id.len() > 200
        || !snapshot
            .dispatch_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        || snapshot.status.trim().is_empty()
    {
        return Err("Connecter dispatch 响应缺少 dispatchId/status。".into());
    }
    Ok(snapshot)
}

fn http_error(action: &str, response: &HttpResponse) -> String {
    let detail = serde_json::from_str::<Value>(&response.body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| response.body.chars().take(500).collect());
    format!("{action}失败（HTTP {}）：{detail}", response.status)
}

fn curl_command_args() -> [&'static str; 2] {
    ["--config", "-"]
}

async fn curl_request(
    profile: &ConnecterProviderProfile,
    method: &str,
    path: &str,
    extra_header: Option<(&str, &str)>,
    body: Option<&Value>,
) -> AppResult<HttpResponse> {
    let base_url = normalize_base_url(&profile.base_url)?;
    let url = format!("{base_url}{path}");
    let config = build_curl_config(method, &url, &profile.bearer_token, extra_header, body);
    let mut command = Command::new("curl");
    command
        .args(curl_command_args())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| format!("无法启动系统 curl：{e}"))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "无法写入 curl 配置。".to_string())?;
    stdin
        .write_all(config.as_bytes())
        .await
        .map_err(|e| format!("写入 curl 配置失败：{e}"))?;
    stdin.shutdown().await.map_err(|e| e.to_string())?;
    drop(stdin);
    let output = child
        .wait_with_output()
        .await
        .map_err(|e| format!("等待 curl 失败：{e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Connecter HTTP 请求失败：{}",
            stderr.trim().chars().take(500).collect::<String>()
        ));
    }
    parse_curl_output(&String::from_utf8_lossy(&output.stdout))
}

fn build_curl_config(
    method: &str,
    url: &str,
    bearer: &str,
    extra_header: Option<(&str, &str)>,
    body: Option<&Value>,
) -> String {
    let mut lines = vec![
        "silent".to_string(),
        "show-error".to_string(),
        format!("request = {}", curl_quote(method)),
        format!("url = {}", curl_quote(url)),
        format!(
            "header = {}",
            curl_quote(&format!("Authorization: Bearer {bearer}"))
        ),
        "header = \"Content-Type: application/json\"".to_string(),
        "connect-timeout = \"5\"".to_string(),
        "max-time = \"15\"".to_string(),
        "write-out = \"\\n%{http_code}\"".to_string(),
    ];
    if let Some((name, value)) = extra_header {
        lines.push(format!(
            "header = {}",
            curl_quote(&format!("{name}: {value}"))
        ));
    }
    if let Some(value) = body {
        lines.push(format!("data-binary = {}", curl_quote(&value.to_string())));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn curl_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn parse_curl_output(output: &str) -> AppResult<HttpResponse> {
    let (body, status) = output
        .rsplit_once('\n')
        .ok_or_else(|| "curl 响应缺少 HTTP 状态码。".to_string())?;
    let status = status
        .trim()
        .parse::<u16>()
        .map_err(|_| "curl HTTP 状态码无效。".to_string())?;
    Ok(HttpResponse {
        status,
        body: body.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Mutex;
    use std::thread;

    fn profile(base_url: String) -> ConnecterProviderProfile {
        ConnecterProviderProfile {
            member_id: "member-a".into(),
            base_url,
            bearer_token: "secret-service-token".into(),
            group_ref: "wp:canary:group-a".into(),
            target_subject_id: "runner:windows11".into(),
            env: "canary".into(),
        }
    }

    fn read_request(stream: &mut std::net::TcpStream) -> String {
        let mut bytes = Vec::new();
        let mut buf = [0u8; 2048];
        loop {
            let n = stream.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&buf[..n]);
            if let Some(header_end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&bytes[..header_end + 4]);
                let content_length = header
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if bytes.len() >= header_end + 4 + content_length {
                    break;
                }
            }
        }
        String::from_utf8(bytes).unwrap()
    }

    fn respond(stream: &mut std::net::TcpStream, status: &str, body: &str) {
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        stream.flush().unwrap();
    }

    #[test]
    fn validates_and_normalizes_profile() {
        let got = validate_profile_input(
            Some(" https://connecter.example/ "),
            Some("token"),
            Some("wp:canary:g"),
            Some("subject"),
            Some("canary"),
        )
        .unwrap();
        assert_eq!(got.base_url, "https://connecter.example");
        assert_eq!(got.group_ref, "wp:canary:g");
        assert!(normalize_base_url("file:///tmp/x").is_err());
        assert!(normalize_base_url("https://example.test/?token=x").is_err());
        assert!(normalize_base_url("https://user:pass@example.test").is_err());
        assert!(normalize_base_url("http:///missing-host").is_err());
        assert!(
            validate_profile_input(Some("http://x"), None, Some("g"), Some("s"), Some("e"))
                .is_err()
        );
        assert!(validate_profile_input(
            Some("http://x"),
            Some("token"),
            Some("canary/g"),
            Some("subject"),
            Some("canary")
        )
        .is_err());
    }

    #[test]
    fn parses_completed_content_shapes_and_failures() {
        let string_snapshot = parse_snapshot(
            r#"{"dispatchId":"d","status":"completed","result":{"content":"hello"}}"#,
        )
        .unwrap();
        assert_eq!(completed_text(&string_snapshot).unwrap(), "hello");
        let object_snapshot = parse_snapshot(
            r#"{"dispatchId":"d","status":"completed","result":{"content":{"text":"world","usage":1}}}"#,
        )
        .unwrap();
        assert_eq!(completed_text(&object_snapshot).unwrap(), "world");
        assert!(parse_snapshot(r#"{"status":"queued"}"#).is_err());
        assert!(parse_snapshot(r#"{"dispatchId":"../escape","status":"queued"}"#).is_err());
    }

    #[test]
    fn bearer_never_enters_curl_argv() {
        assert_eq!(curl_command_args(), ["--config", "-"]);
        assert!(!curl_command_args().join(" ").contains("secret"));
        let config = build_curl_config(
            "POST",
            "http://127.0.0.1/v2/dispatches",
            "secret-service-token",
            Some(("Idempotency-Key", "run-1")),
            Some(&json!({"writeBack": false})),
        );
        assert!(config.contains("Authorization: Bearer secret-service-token"));
        assert!(config.contains("Idempotency-Key: run-1"));
    }

    #[tokio::test]
    async fn real_curl_create_poll_and_cancel_contract() {
        if std::process::Command::new("curl")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("skip real curl contract: curl unavailable");
            return;
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::<String>::new()));
        let captured = requests.clone();
        let server = thread::spawn(move || {
            for index in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_request(&mut stream);
                captured.lock().unwrap().push(request);
                match index {
                    0 => respond(
                        &mut stream,
                        "202 Accepted",
                        r#"{"dispatchId":"dispatch-1","status":"queued","writeBack":false}"#,
                    ),
                    1 => respond(
                        &mut stream,
                        "200 OK",
                        r#"{"dispatchId":"dispatch-1","status":"completed","result":{"content":{"text":"REMOTE_OK"}}}"#,
                    ),
                    _ => respond(
                        &mut stream,
                        "202 Accepted",
                        r#"{"dispatchId":"dispatch-1","status":"cancelled"}"#,
                    ),
                }
            }
        });
        let profile = profile(format!("http://{address}"));
        let created = create_dispatch(
            &profile,
            "run-123",
            "group-local",
            "member-a",
            "Canary Group",
            "do work",
        )
        .await
        .unwrap();
        let token = Arc::new(AtomicBool::new(false));
        let final_text = poll_until_terminal(
            &profile,
            created,
            Duration::from_secs(2),
            Duration::from_millis(1),
            &token,
        )
        .await
        .unwrap();
        assert_eq!(final_text, "REMOTE_OK");
        cancel_dispatch(&profile, "dispatch-1").await.unwrap();
        server.join().unwrap();

        let got = requests.lock().unwrap();
        assert_eq!(got.len(), 3);
        assert!(got[0].starts_with("POST /v2/dispatches "));
        assert!(got[0].contains("Authorization: Bearer secret-service-token"));
        assert!(got[0].contains("Idempotency-Key: run-123"));
        assert!(got[0].contains("\"writeBack\":false"));
        assert!(got[0].contains("\"targetSubjectId\":\"runner:windows11\""));
        assert!(got[1].starts_with("GET /v2/dispatches/dispatch-1 "));
        assert!(got[2].starts_with("POST /v2/dispatches/dispatch-1/cancel "));
    }
}
