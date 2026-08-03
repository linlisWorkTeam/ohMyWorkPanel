/** Distance from bottom (px) counted as "stuck to bottom". */
export const BOTTOM_THRESHOLD_PX = 80;

export function isNearBottom(
  scrollTop: number,
  scrollHeight: number,
  clientHeight: number,
  threshold = BOTTOM_THRESHOLD_PX,
): boolean {
  return scrollHeight - scrollTop - clientHeight < threshold;
}

/** Historical agent replies stay collapsed; live streams stay open. */
export function agentReplyDefaultOpen(streaming: boolean): boolean {
  return streaming;
}

export function previewAgentReply(text: string, maxLen = 48): string {
  const trimmed = text.replace(/\s+/g, " ").trim();
  if (!trimmed) return "Agent 答复";
  if (trimmed.length <= maxLen) return trimmed;
  return `${trimmed.slice(0, maxLen)}…`;
}

/** Prefer final channel text; fall back to plain content. */
export function extractReplyPreview(content: string, maxLen = 48): string {
  try {
    const parsed = JSON.parse(content) as { parts?: Array<{ channel?: string; text?: string }> };
    if (parsed?.parts && Array.isArray(parsed.parts)) {
      const finals = parsed.parts
        .filter((p) => p.channel === "final" && (p.text ?? "").trim())
        .map((p) => p.text!.trim());
      if (finals.length) return previewAgentReply(finals.join("\n"), maxLen);
      const any = parsed.parts.map((p) => (p.text ?? "").trim()).filter(Boolean);
      if (any.length) return previewAgentReply(any[any.length - 1], maxLen);
    }
  } catch {
    /* plain text */
  }
  return previewAgentReply(content, maxLen);
}
