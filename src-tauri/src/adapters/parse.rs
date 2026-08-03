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

    // Cursor stream-json: never echo system/user envelopes into the agent bubble.
    if type_hint == "system" || type_hint == "user" {
        return ParsedEvent {
            channel: "final".into(),
            text: String::new(),
            session_id,
            mode: DeltaMode::Append,
        };
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
}
