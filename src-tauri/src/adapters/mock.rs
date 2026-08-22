/// Mock stream as (channel, text) chunks for UI smoke tests.
pub const STREAM_EVENTS: &[(&str, &str)] = &[
    ("thinking", "先快速扫一眼任务上下文…"),
    ("artifact", "$ inspect\n(mock) workspace ready\n"),
    ("final", "已收到任务。"),
    ("final", "我会根据当前群聊上下文进行处理。"),
    ("final", "（这是本地模拟 Agent 的流式回复。）"),
];

/// Backward-compatible plain chunks (final only).
pub const STREAM_CHUNKS: &[&str] = &[
    "已收到任务。",
    "我会根据当前群聊上下文进行处理。",
    "（这是本地模拟 Agent 的流式回复。）",
];
