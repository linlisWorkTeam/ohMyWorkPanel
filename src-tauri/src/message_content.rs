use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContentPart {
    pub channel: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContentDoc {
    pub v: u32,
    pub parts: Vec<ContentPart>,
}

pub fn parse_content_doc(existing: &str) -> Option<ContentDoc> {
    let trimmed = existing.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    let value: Value = serde_json::from_str(trimmed).ok()?;
    if value.get("v")?.as_u64()? != 1 {
        return None;
    }
    if !value.get("parts")?.is_array() {
        return None;
    }
    serde_json::from_value(value).ok()
}

pub fn append_channel_delta(existing: &str, channel: &str, delta: &str) -> String {
    apply_channel_delta(existing, channel, delta, false)
}

/// When `replace` is true, overwrite that channel's text instead of appending.
pub fn apply_channel_delta(existing: &str, channel: &str, delta: &str, replace: bool) -> String {
    let channel = normalize_channel(channel);
    let mut doc = match parse_content_doc(existing) {
        Some(doc) => doc,
        None if existing.trim().is_empty() => ContentDoc {
            v: 1,
            parts: Vec::new(),
        },
        None => ContentDoc {
            v: 1,
            parts: vec![ContentPart {
                channel: "final".into(),
                text: existing.to_string(),
            }],
        },
    };
    if let Some(part) = doc.parts.iter_mut().find(|p| p.channel == channel) {
        if replace {
            part.text = delta.to_string();
        } else {
            part.text.push_str(delta);
        }
    } else {
        doc.parts.push(ContentPart {
            channel,
            text: delta.to_string(),
        });
    }
    serde_json::to_string(&doc).unwrap_or_else(|_| delta.to_string())
}

pub fn parts_to_plain_text(content: &str) -> String {
    match parse_content_doc(content) {
        Some(doc) => {
            if let Some(final_part) = doc.parts.iter().find(|p| p.channel == "final") {
                if !final_part.text.is_empty() {
                    return final_part.text.clone();
                }
            }
            doc.parts
                .iter()
                .map(|p| p.text.as_str())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        }
        None => content.to_string(),
    }
}

pub fn normalize_channel(channel: &str) -> String {
    match channel.trim().to_ascii_lowercase().as_str() {
        "thinking" | "reasoning" | "thought" => "thinking".into(),
        "artifact" | "tool" | "tool_result" | "command" => "artifact".into(),
        _ => "final".into(),
    }
}

/// List/hot-window payload: keep final text only; flag heavy channels for lazy fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListContentProjection {
    pub content: String,
    pub has_thinking: bool,
    pub has_artifact: bool,
}

pub fn project_content_for_list(content: &str) -> ListContentProjection {
    match parse_content_doc(content) {
        Some(doc) => {
            let has_thinking = doc
                .parts
                .iter()
                .any(|p| p.channel == "thinking" && !p.text.trim().is_empty());
            let has_artifact = doc
                .parts
                .iter()
                .any(|p| p.channel == "artifact" && !p.text.trim().is_empty());
            let finals: Vec<ContentPart> = doc
                .parts
                .into_iter()
                .filter(|p| p.channel == "final")
                .collect();
            let projected = if finals.is_empty() && !has_thinking && !has_artifact {
                content.to_string()
            } else if finals.len() == 1 && !has_thinking && !has_artifact {
                // Preserve single-final JSON shape for clients that parse parts.
                serde_json::to_string(&ContentDoc {
                    v: 1,
                    parts: finals,
                })
                .unwrap_or_else(|_| content.to_string())
            } else {
                serde_json::to_string(&ContentDoc {
                    v: 1,
                    parts: finals,
                })
                .unwrap_or_else(|_| {
                    // Fallback: plain final text if any.
                    parts_to_plain_text(content)
                })
            };
            ListContentProjection {
                content: projected,
                has_thinking,
                has_artifact,
            }
        }
        None => ListContentProjection {
            content: content.to_string(),
            has_thinking: false,
            has_artifact: false,
        },
    }
}

pub fn extract_channel_text(content: &str, channel: &str) -> String {
    let channel = normalize_channel(channel);
    match parse_content_doc(content) {
        Some(doc) => doc
            .parts
            .into_iter()
            .find(|p| p.channel == channel)
            .map(|p| p.text)
            .unwrap_or_default(),
        None if channel == "final" => content.to_string(),
        None => String::new(),
    }
}

pub fn is_lazy_channel(channel: &str) -> bool {
    matches!(normalize_channel(channel).as_str(), "thinking" | "artifact")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_channels_and_upgrades_legacy() {
        let mut content = String::new();
        content = append_channel_delta(&content, "thinking", "t1");
        content = append_channel_delta(&content, "final", "hello");
        content = append_channel_delta(&content, "final", " world");
        let doc = parse_content_doc(&content).unwrap();
        assert_eq!(doc.parts.len(), 2);
        assert_eq!(doc.parts[0].channel, "thinking");
        assert_eq!(doc.parts[1].text, "hello world");
        assert_eq!(parts_to_plain_text(&content), "hello world");

        let upgraded = append_channel_delta("legacy text", "artifact", " more");
        let doc = parse_content_doc(&upgraded).unwrap();
        assert_eq!(doc.parts[0].channel, "final");
        assert_eq!(doc.parts[0].text, "legacy text");
        assert_eq!(doc.parts[1].channel, "artifact");
    }

    #[test]
    fn project_list_strips_thinking_and_artifact() {
        let mut content = String::new();
        content = append_channel_delta(&content, "thinking", "secret think");
        content = append_channel_delta(&content, "artifact", "tool out");
        content = append_channel_delta(&content, "final", "answer");
        let proj = project_content_for_list(&content);
        assert!(proj.has_thinking);
        assert!(proj.has_artifact);
        assert!(!proj.content.contains("secret think"));
        assert!(!proj.content.contains("tool out"));
        assert!(proj.content.contains("answer"));
        assert_eq!(extract_channel_text(&content, "thinking"), "secret think");
        assert_eq!(extract_channel_text(&content, "artifact"), "tool out");
    }
}
