use serde_json::Value;

pub fn parse_agent_line(line: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(line) {
        return extract_text(&value).unwrap_or_default();
    }
    line.to_string()
}

pub fn extract_text(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in ["delta", "text", "content"] {
                if let Some(Value::String(text)) = map.get(key) {
                    if !text.trim().is_empty() {
                        return Some(text.clone());
                    }
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
}
