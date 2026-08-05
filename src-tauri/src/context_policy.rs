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

#[cfg(test)]
mod tests {
    use super::*;

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
}
