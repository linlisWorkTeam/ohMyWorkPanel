use chrono::{Local, TimeZone};

/// Format a history line with local timestamp so chatbot can reason about "刚才/今天".
/// `created_at_ms` is epoch millis (same as `db::now()`).
pub fn format_history_line(display_name: &str, content: &str, created_at_ms: i64) -> String {
    let ts = match Local.timestamp_millis_opt(created_at_ms) {
        chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
        _ => created_at_ms.to_string(),
    };
    format!("[{ts}] {display_name}: {content}")
}

/// Resolve how many recent messages to load into the prompt window.
/// - Project agents: `project_limit` (default 40).
/// - Chat groups or chatbot members: `chat_limit` (default 12) — smaller native window, no summary/RAG.
pub fn effective_context_message_limit(
    group_kind: &str,
    agent_kind: &str,
    project_limit: i64,
    chat_limit: i64,
) -> i64 {
    let project = project_limit.max(5);
    let chat = chat_limit.clamp(5, 40);
    if agent_kind == "chatbot" || group_kind == "chat" {
        chat
    } else {
        project
    }
}

/// Soft character budget for history dump (bytes/UTF-8 len used by scheduler trim loop).
pub fn effective_history_char_budget(group_kind: &str, agent_kind: &str) -> usize {
    if agent_kind == "chatbot" || group_kind == "chat" {
        8_000
    } else {
        24_000
    }
}

/// Extra messages loaded beyond the keep window to fold into a summary when overflowing.
pub const CHAT_FOLD_BATCH: usize = 24;

/// Split oldest→newest items into (to_fold, keep) when len > keep_limit.
pub fn split_rolling_window<T>(items: &[T], keep_limit: usize) -> (&[T], &[T]) {
    if keep_limit == 0 || items.len() <= keep_limit {
        (&[], items)
    } else {
        let fold_at = items.len() - keep_limit;
        (&items[..fold_at], &items[fold_at..])
    }
}

/// Compose persisted summary + recent raw lines for the native window.
pub fn compose_window_with_summary(summary: &str, recent_chat: &str) -> String {
    let summary = summary.trim();
    let recent = recent_chat.trim();
    match (summary.is_empty(), recent.is_empty()) {
        (true, true) => String::new(),
        (false, true) => format!("【历史摘要】\n{summary}"),
        (true, false) => recent.to_string(),
        (false, false) => {
            format!("【历史摘要】\n{summary}\n\n【最近群聊（从旧到新）】\n{recent}")
        }
    }
}

/// Prompt body asking the model to compress folded turns (used with chatbot HTTP).
pub fn build_fold_summary_prompt(existing_summary: &str, folded_lines: &str) -> String {
    let existing = existing_summary.trim();
    let folded = folded_lines.trim();
    format!(
        "请把以下群聊压缩成简短中文摘要（保留人物、议题、结论与未决事项），不超过200字，不要条列编号外的废话。\n\n【已有摘要】\n{}\n\n【待压缩对话】\n{}\n\n只输出摘要正文。",
        if existing.is_empty() {
            "（无）"
        } else {
            existing
        },
        folded
    )
}

/// Offline fallback when LLM fold fails: keep old summary + first lines truncated.
pub fn extractive_fold_summary(existing_summary: &str, folded_lines: &str, max_chars: usize) -> String {
    let mut parts = Vec::new();
    let existing = existing_summary.trim();
    if !existing.is_empty() {
        parts.push(existing.to_string());
    }
    let folded = folded_lines.trim();
    if !folded.is_empty() {
        let snippet: String = folded.chars().take(max_chars.min(800)).collect();
        parts.push(format!("（摘录）{snippet}"));
    }
    let joined = parts.join("\n");
    if joined.chars().count() <= max_chars {
        joined
    } else {
        joined.chars().take(max_chars).collect::<String>() + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_line_includes_timestamp() {
        // 2026-08-05 08:00:00 UTC → local may differ; assert bracket + name + body shape.
        let line = format_history_line("Alice", "你好", 1_786_204_800_000);
        assert!(line.contains("Alice: 你好"), "{line}");
        assert!(line.starts_with('['), "{line}");
        assert!(line.contains(']'), "{line}");
    }

    #[test]
    fn project_agent_uses_project_limit() {
        assert_eq!(
            effective_context_message_limit("project", "agent", 40, 12),
            40
        );
    }

    #[test]
    fn chat_group_uses_chat_limit() {
        assert_eq!(
            effective_context_message_limit("chat", "agent", 40, 12),
            12
        );
    }

    #[test]
    fn chatbot_uses_chat_limit_even_in_project() {
        assert_eq!(
            effective_context_message_limit("project", "chatbot", 40, 12),
            12
        );
    }

    #[test]
    fn chat_budget_is_tighter() {
        assert_eq!(effective_history_char_budget("chat", "chatbot"), 8_000);
        assert_eq!(effective_history_char_budget("project", "agent"), 24_000);
    }

    #[test]
    fn rolling_split_folds_overflow_only() {
        let items = [1, 2, 3, 4, 5];
        let (fold, keep) = split_rolling_window(&items, 3);
        assert_eq!(fold, &[1, 2]);
        assert_eq!(keep, &[3, 4, 5]);
        let (fold2, keep2) = split_rolling_window(&items, 5);
        assert!(fold2.is_empty());
        assert_eq!(keep2, &items);
    }

    #[test]
    fn compose_includes_summary_and_recent() {
        let text = compose_window_with_summary("聊过 Embedding", "[t] A: hi");
        assert!(text.contains("【历史摘要】"));
        assert!(text.contains("Embedding"));
        assert!(text.contains("【最近群聊"));
        assert!(text.contains("A: hi"));
    }
}
