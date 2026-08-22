export type MessageChannel = "thinking" | "artifact" | "final";

export interface MessagePart {
  channel: MessageChannel;
  text: string;
}

export interface MessageContentDoc {
  v: 1;
  parts: MessagePart[];
}

function normalizeChannel(channel: string | null | undefined): MessageChannel {
  const value = (channel ?? "final").trim().toLowerCase();
  if (value === "thinking" || value === "reasoning" || value === "thought") return "thinking";
  if (value === "artifact" || value === "tool" || value === "tool_result" || value === "command") {
    return "artifact";
  }
  return "final";
}

export function parseMessageContent(content: string): MessageContentDoc | null {
  const trimmed = content.trim();
  if (!trimmed.startsWith("{")) return null;
  try {
    const value = JSON.parse(trimmed) as { v?: number; parts?: MessagePart[] };
    if (value?.v !== 1 || !Array.isArray(value.parts)) return null;
    return {
      v: 1,
      parts: value.parts.map((part) => ({
        channel: normalizeChannel(part.channel),
        text: typeof part.text === "string" ? part.text : "",
      })),
    };
  } catch {
    return null;
  }
}

export function appendChannelDelta(
  existing: string,
  channel: string | null | undefined,
  delta: string,
  replace = false,
): string {
  const normalized = normalizeChannel(channel);
  let doc = parseMessageContent(existing);
  if (!doc) {
    doc = existing.trim()
      ? { v: 1, parts: [{ channel: "final", text: existing }] }
      : { v: 1, parts: [] };
  }
  const part = doc.parts.find((item) => item.channel === normalized);
  if (part) {
    part.text = replace ? delta : part.text + delta;
  } else {
    doc.parts.push({ channel: normalized, text: delta });
  }
  return JSON.stringify(doc);
}

export function partsToPlainText(content: string): string {
  const doc = parseMessageContent(content);
  if (!doc) return content;
  const finalPart = doc.parts.find((part) => part.channel === "final");
  if (finalPart?.text) return finalPart.text;
  return doc.parts.map((part) => part.text).filter(Boolean).join("\n");
}

export function hasRenderableContent(content: string): boolean {
  const doc = parseMessageContent(content);
  if (!doc) return content.trim().length > 0;
  return doc.parts.some((part) => part.text.trim().length > 0);
}

export function isLazyMessageChannel(channel: string | null | undefined): boolean {
  const value = (channel ?? "").trim().toLowerCase();
  return value === "thinking" || value === "reasoning" || value === "thought"
    || value === "artifact" || value === "tool" || value === "tool_result" || value === "command";
}

/** Strip thinking/artifact bodies for local display state (mirrors server list projection). */
export function projectContentForList(content: string): {
  content: string;
  hasThinking: boolean;
  hasArtifact: boolean;
} {
  const doc = parseMessageContent(content);
  if (!doc) {
    return { content, hasThinking: false, hasArtifact: false };
  }
  const hasThinking = doc.parts.some((p) => p.channel === "thinking" && p.text.trim());
  const hasArtifact = doc.parts.some((p) => p.channel === "artifact" && p.text.trim());
  const finals = doc.parts.filter((p) => p.channel === "final");
  return {
    content: JSON.stringify({ v: 1, parts: finals }),
    hasThinking,
    hasArtifact,
  };
}
