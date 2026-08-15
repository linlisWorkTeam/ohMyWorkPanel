use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaMode {
    Append,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEvent {
    pub channel: String,
    pub text: String,
    pub session_id: Option<String>,
    pub mode: DeltaMode,
}

pub fn parse_agent_line(line: &str) -> String {
    parse_agent_event(line).text
}

pub fn parse_agent_event(line: &str) -> ParsedEvent {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return empty_event();
    }
    let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
        return ParsedEvent {
            channel: "final".into(),
            text: line.to_string(),
            session_id: None,
            mode: DeltaMode::Append,
        };
    };

    let session_id = extract_session_id(&value);
    let type_hint = value
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let subtype = value
        .get("subtype")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // OpenClaw `agent --json` final envelope: { runId, status, result: { payloads:[{text}] } }
    if let Some(event) = parse_openclaw_envelope(&value, session_id.clone()) {
        return event;
    }

    // Cursor stream-json: never echo system/user envelopes into the agent bubble.
    if type_hint == "system" || type_hint == "user" {
        return ParsedEvent {
            channel: "final".into(),
            text: String::new(),
            session_id,
            mode: DeltaMode::Append,
        };
    }

    // Codex CLI `exec --json` (JSONL): coarse events, not token deltas.
    // See: thread/turn lifecycle + item.completed with nested item.text.
    if type_hint == "thread.started" || type_hint == "turn.completed" {
        return ParsedEvent {
            channel: "final".into(),
            text: String::new(),
            session_id,
            mode: DeltaMode::Append,
        };
    }
    if type_hint == "error" || type_hint == "turn.failed" {
        let msg = value
            .pointer("/error/message")
            .and_then(|v| v.as_str())
            .or_else(|| value.get("message").and_then(|v| v.as_str()))
            .unwrap_or("Codex 回合失败")
            .to_string();
        return ParsedEvent {
            channel: "thinking".into(),
            text: format!("{msg}\n"),
            session_id,
            mode: DeltaMode::Append,
        };
    }
    if type_hint == "turn.started" {
        return ParsedEvent {
            channel: "thinking".into(),
            text: "Codex 回合已开始…\n".into(),
            session_id,
            mode: DeltaMode::Append,
        };
    }
    if type_hint.starts_with("item.") {
        if let Some(item) = value.get("item") {
            let item_type = item
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let text = item
                .get("text")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or_else(|| extract_text(item))
                .unwrap_or_default();
            if text.is_empty() && !type_hint.ends_with("completed") {
                return empty_event();
            }
            if item_type.contains("reason") || item_type == "thinking" {
                return ParsedEvent {
                    channel: "thinking".into(),
                    text,
                    session_id,
                    mode: DeltaMode::Append,
                };
            }
            if item_type.contains("tool")
                || item_type.contains("command")
                || item_type == "file_change"
            {
                return ParsedEvent {
                    channel: "artifact".into(),
                    text: if text.is_empty() {
                        format!("[{item_type}]")
                    } else {
                        text
                    },
                    session_id,
                    mode: DeltaMode::Append,
                };
            }
            // agent_message / message / default → final answer
            if type_hint.ends_with("completed") || !text.is_empty() {
                return ParsedEvent {
                    channel: "final".into(),
                    text,
                    session_id,
                    mode: if type_hint.ends_with("completed") {
                        DeltaMode::Replace
                    } else {
                        DeltaMode::Append
                    },
                };
            }
        }
        return empty_event();
    }

    if type_hint == "result" {
        let text = value
            .get("result")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        return ParsedEvent {
            channel: "final".into(),
            text,
            session_id,
            mode: DeltaMode::Replace,
        };
    }

    if type_hint == "thinking" || type_hint == "reasoning" {
        if subtype == "completed" {
            return ParsedEvent {
                channel: "thinking".into(),
                text: String::new(),
                session_id,
                mode: DeltaMode::Append,
            };
        }
        let text = value
            .get("text")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| extract_text(&value))
            .unwrap_or_default();
        return ParsedEvent {
            channel: "thinking".into(),
            text,
            session_id,
            mode: DeltaMode::Append,
        };
    }

    if type_hint.contains("tool")
        || type_hint == "artifact"
        || type_hint == "command"
        || type_hint == "shell"
    {
        return ParsedEvent {
            channel: "artifact".into(),
            text: extract_text(&value).unwrap_or_default(),
            session_id,
            mode: DeltaMode::Append,
        };
    }

    if type_hint == "assistant" {
        let text = extract_assistant_text(&value).unwrap_or_default();
        // Cursor partial chunks include timestamp_ms; the final full snapshot often does not.
        let mode = if value.get("timestamp_ms").is_some() || subtype == "delta" {
            DeltaMode::Append
        } else {
            DeltaMode::Replace
        };
        return ParsedEvent {
            channel: "final".into(),
            text,
            session_id,
            mode,
        };
    }

    let channel = classify_channel(&value);
    ParsedEvent {
        channel,
        text: extract_text(&value).unwrap_or_default(),
        session_id,
        mode: DeltaMode::Append,
    }
}

fn empty_event() -> ParsedEvent {
    ParsedEvent {
        channel: "final".into(),
        text: String::new(),
        session_id: None,
        mode: DeltaMode::Append,
    }
}

/// OpenClaw CLI JSON result object (pretty or compact).
///
/// Two shapes seen in the wild:
/// 1) Wrapped: `{ runId, status, result: { payloads:[{text}], meta } }`
/// 2) Bare result (often on stderr after gateway→embedded fallback):
///    `{ payloads:[{text}], meta }`
fn parse_openclaw_envelope(value: &Value, session_id: Option<String>) -> Option<ParsedEvent> {
    let has_run_id = value.get("runId").is_some() || value.get("run_id").is_some();
    let result_obj = value.get("result").filter(|v| v.is_object());
    let payloads = result_obj
        .and_then(|r| r.get("payloads"))
        .or_else(|| value.get("payloads"))
        .and_then(|v| v.as_array());
    // Bare `{payloads, meta}` has no runId — still OpenClaw.
    if !has_run_id && payloads.is_none() && result_obj.is_none() {
        return None;
    }
    if payloads.is_none() && result_obj.is_none() {
        return None;
    }
    // Prefer payloads[].text; ignore huge meta/systemPromptReport.
    let mut texts = Vec::new();
    if let Some(items) = payloads {
        for item in items {
            if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                if !t.is_empty() {
                    texts.push(t.to_string());
                }
            }
        }
    }
    let text = if texts.is_empty() {
        // Fallbacks without dumping the whole envelope.
        result_obj
            .and_then(|r| r.get("text"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| {
                value
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .filter(|s| *s != "completed" && *s != "ok")
                    .map(str::to_string)
            })
            .unwrap_or_default()
    } else {
        texts.join("\n\n")
    };
    // Never echo raw envelope JSON into the chat bubble.
    if looks_like_openclaw_envelope_text(&text) {
        return None;
    }
    let sid = session_id.or_else(|| {
        result_obj
            .and_then(|r| r.pointer("/meta/agentMeta/sessionId"))
            .or_else(|| value.pointer("/meta/agentMeta/sessionId"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    });
    Some(ParsedEvent {
        channel: "final".into(),
        text,
        session_id: sid,
        mode: DeltaMode::Replace,
    })
}

fn looks_like_openclaw_envelope_text(text: &str) -> bool {
    let t = text.trim_start();
    if !t.starts_with('{') {
        return false;
    }
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        return v.get("runId").is_some()
            || v.get("run_id").is_some()
            || (v.get("payloads").is_some() && v.get("meta").is_some())
            || (v.get("result").and_then(|r| r.get("payloads")).is_some());
    }
    t.contains("\"runId\"") && t.contains("\"payloads\"")
}

/// Pull the first balanced `{...}` JSON object from mixed CLI stderr/stdout.
pub fn extract_json_object_from_mixed(raw: &str) -> Option<&str> {
    let bytes = raw.as_bytes();
    let start = raw.find('{')?;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&raw[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse OpenClaw output that may sit on stderr with gateway noise prefixed.
pub fn parse_openclaw_mixed_output(raw: &str) -> Option<ParsedEvent> {
    let json = extract_json_object_from_mixed(raw)?;
    let event = parse_agent_event(json);
    if event.text.is_empty() {
        return None;
    }
    if event.text.contains("\"runId\"") || event.text.contains("\"payloads\"") {
        return None;
    }
    Some(event)
}

fn extract_assistant_text(value: &Value) -> Option<String> {
    let message = value.get("message")?;
    let content = message.get("content")?;
    match content {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Array(parts) => {
            let mut out = String::new();
            for part in parts {
                if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                    out.push_str(text);
                } else if let Some(text) = extract_text(part) {
                    out.push_str(&text);
                }
            }
            if out.is_empty() {
                None
            } else {
                Some(out)
            }
        }
        _ => extract_text(value),
    }
}

fn classify_channel(value: &Value) -> String {
    let mut hints = Vec::new();
    collect_hints(value, &mut hints, 0);
    let joined = hints.join(" ").to_ascii_lowercase();
    if joined.contains("thinking") || joined.contains("reasoning") || joined.contains("thought") {
        return "thinking".into();
    }
    if joined.contains("tool")
        || joined.contains("artifact")
        || joined.contains("command")
        || joined.contains("shell")
    {
        return "artifact".into();
    }
    "final".into()
}

fn collect_hints(value: &Value, out: &mut Vec<String>, depth: usize) {
    if depth > 4 {
        return;
    }
    match value {
        Value::Object(map) => {
            for key in ["type", "role", "kind", "subtype", "name", "event"] {
                if let Some(Value::String(s)) = map.get(key) {
                    out.push(format!("{key}:{s}"));
                    out.push(s.clone());
                }
            }
            for value in map.values() {
                collect_hints(value, out, depth + 1);
            }
        }
        Value::Array(values) => {
            for value in values.iter().take(8) {
                collect_hints(value, out, depth + 1);
            }
        }
        _ => {}
    }
}

pub fn extract_text(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in ["delta", "text", "content", "result"] {
                if let Some(Value::String(text)) = map.get(key) {
                    if !text.is_empty() {
                        return Some(text.clone());
                    }
                }
            }
            if let Some(nested) = map.get("delta") {
                if let Some(text) = extract_text(nested) {
                    return Some(text);
                }
            }
            if let Some(Value::Array(parts)) = map.get("content") {
                let mut out = String::new();
                for part in parts {
                    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                        out.push_str(text);
                    } else if let Some(text) = extract_text(part) {
                        out.push_str(&text);
                    }
                }
                if !out.is_empty() {
                    return Some(out);
                }
            }
            for value in map.values() {
                if let Some(text) = extract_text(value) {
                    return Some(text);
                }
            }
            None
        }
        Value::Array(values) => values.iter().find_map(extract_text),
        _ => None,
    }
}

fn extract_session_id(value: &Value) -> Option<String> {
    extract_session_id_inner(value, 0)
}

fn extract_session_id_inner(value: &Value, depth: usize) -> Option<String> {
    if depth > 5 {
        return None;
    }
    match value {
        Value::Object(map) => {
            for key in ["session_id", "sessionId", "chatId", "chat_id", "conversationId"] {
                if let Some(Value::String(id)) = map.get(key) {
                    if !id.trim().is_empty() {
                        return Some(id.clone());
                    }
                }
            }
            if let Some(Value::Object(session)) = map.get("session") {
                if let Some(Value::String(id)) = session.get("id") {
                    if !id.trim().is_empty() {
                        return Some(id.clone());
                    }
                }
            }
            for value in map.values() {
                if let Some(id) = extract_session_id_inner(value, depth + 1) {
                    return Some(id);
                }
            }
            None
        }
        Value::Array(values) => values
            .iter()
            .find_map(|v| extract_session_id_inner(v, depth + 1)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_stream_text_from_nested_json() {
        let value: Value =
            serde_json::from_str(r#"{"type":"item","delta":{"text":"hello"}}"#).unwrap();
        assert_eq!(extract_text(&value).as_deref(), Some("hello"));
    }

    #[test]
    fn parse_agent_line_falls_back_to_raw_text() {
        assert_eq!(parse_agent_line("plain output"), "plain output");
    }

    #[test]
    fn classifies_thinking_and_session() {
        let event = parse_agent_event(
            r#"{"type":"thinking","subtype":"delta","text":"hmm","session_id":"sess-1"}"#,
        );
        assert_eq!(event.channel, "thinking");
        assert_eq!(event.text, "hmm");
        assert_eq!(event.session_id.as_deref(), Some("sess-1"));
        assert_eq!(event.mode, DeltaMode::Append);
    }

    #[test]
    fn classifies_tool_as_artifact() {
        let event = parse_agent_event(r#"{"type":"tool_result","text":"ok"}"#);
        assert_eq!(event.channel, "artifact");
        assert_eq!(event.text, "ok");
    }

    #[test]
    fn skips_user_envelope_and_replaces_final_snapshot() {
        let user = parse_agent_event(
            r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"hi"}]},"session_id":"s"}"#,
        );
        assert!(user.text.is_empty());
        assert_eq!(user.session_id.as_deref(), Some("s"));

        let partial = parse_agent_event(
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hel"}]},"session_id":"s","timestamp_ms":1}"#,
        );
        assert_eq!(partial.text, "Hel");
        assert_eq!(partial.mode, DeltaMode::Append);

        let full = parse_agent_event(
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hello"}]},"session_id":"s"}"#,
        );
        assert_eq!(full.text, "Hello");
        assert_eq!(full.mode, DeltaMode::Replace);

        let result = parse_agent_event(
            r#"{"type":"result","subtype":"success","result":"Hello","session_id":"s"}"#,
        );
        assert_eq!(result.text, "Hello");
        assert_eq!(result.mode, DeltaMode::Replace);
    }

    #[test]
    fn parses_openclaw_run_envelope_payload_text() {
        let raw = r#"{
          "runId": "7a356d8f-6536-4c38-8ee5-8b3aa1e32a0b",
          "status": "ok",
          "summary": "completed",
          "result": {
            "payloads": [{ "text": "在的 ✅\n\n我是 OpenClaw", "mediaUrl": null }],
            "meta": { "agentMeta": { "sessionId": "14603e9e-a615-428f-8cef-7189e5f4d9bc" } }
          }
        }"#;
        let event = parse_agent_event(raw);
        assert_eq!(event.channel, "final");
        assert_eq!(event.mode, DeltaMode::Replace);
        assert!(event.text.starts_with("在的 ✅"));
        assert!(!event.text.contains("runId"));
        assert!(!event.text.contains("systemPromptReport"));
        assert_eq!(
            event.session_id.as_deref(),
            Some("14603e9e-a615-428f-8cef-7189e5f4d9bc")
        );
    }

    #[test]
    fn parses_openclaw_bare_payloads_result_shape() {
        let raw = r#"{
          "payloads": [{ "text": "PONG_OPENCLAW", "mediaUrl": null }],
          "meta": { "agentMeta": { "sessionId": "sess-bare" } }
        }"#;
        let event = parse_agent_event(raw);
        assert_eq!(event.text, "PONG_OPENCLAW");
        assert_eq!(event.session_id.as_deref(), Some("sess-bare"));
        assert!(!looks_like_openclaw_envelope_text(&event.text));
    }

    #[test]
    fn parses_openclaw_json_buried_in_gateway_stderr() {
        let raw = r#"Gateway agent failed; falling back to embedded: Error: gateway closed (1006)
Gateway target: ws://127.0.0.1:18789
{
  "payloads": [{ "text": "PONG_OPENCLAW", "mediaUrl": null }],
  "meta": { "agentMeta": { "sessionId": "s2" } }
}
"#;
        let event = parse_openclaw_mixed_output(raw).expect("extract");
        assert_eq!(event.text, "PONG_OPENCLAW");
        assert!(!event.text.contains("Gateway"));
        assert!(!event.text.contains("runId"));
    }

    #[test]
    fn openclaw_final_must_not_be_raw_envelope() {
        // Regression: canary once stored the whole envelope as final text.
        let leaked = r#"{"runId":"x","status":"ok","result":{"payloads":[{"text":"hi"}]}}"#;
        let event = parse_agent_event(leaked);
        assert_eq!(event.text, "hi");
        assert!(!looks_like_openclaw_envelope_text(&event.text));
    }

    #[test]
    fn parses_codex_jsonl_item_completed() {
        let started = parse_agent_event(r#"{"type":"turn.started"}"#);
        assert_eq!(started.channel, "thinking");
        assert!(!started.text.is_empty());

        let msg = parse_agent_event(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"hi"}}"#,
        );
        assert_eq!(msg.channel, "final");
        assert_eq!(msg.text, "hi");
        assert_eq!(msg.mode, DeltaMode::Replace);

        let lifecycle = parse_agent_event(
            r#"{"type":"thread.started","thread_id":"t1"}"#,
        );
        assert!(lifecycle.text.is_empty());

        let failed = parse_agent_event(
            r#"{"type":"turn.failed","error":{"message":"unexpected status 401"}}"#,
        );
        assert_eq!(failed.channel, "thinking");
        assert!(failed.text.contains("401"));
    }
}
