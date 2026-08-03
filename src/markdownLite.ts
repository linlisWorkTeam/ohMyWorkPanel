/** Tiny Markdown → safe HTML for chat bubbles (no external deps). */

function escapeHtml(raw: string): string {
  return raw
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function inlineFormat(escaped: string): string {
  return escaped
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/(^|[\s(])\*([^*\n]+)\*(?=[\s).,!?:;]|$)/g, "$1<em>$2</em>")
    .replace(
      /\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)/g,
      '<a href="$2" target="_blank" rel="noreferrer noopener">$1</a>',
    );
}

/** Convert a Markdown-ish string to sanitized HTML. */
export function markdownToHtml(source: string): string {
  const normalized = source.replace(/\r\n/g, "\n");
  const parts: string[] = [];
  const fence = /```([a-zA-Z0-9_-]*)\n([\s\S]*?)```/g;
  let last = 0;
  let match: RegExpExecArray | null;
  while ((match = fence.exec(normalized))) {
    parts.push(renderBlocks(normalized.slice(last, match.index)));
    const lang = match[1] ? ` data-lang="${escapeHtml(match[1])}"` : "";
    parts.push(`<pre class="md-code"${lang}><code>${escapeHtml(match[2].replace(/\n$/, ""))}</code></pre>`);
    last = match.index + match[0].length;
  }
  parts.push(renderBlocks(normalized.slice(last)));
  return parts.join("");
}

function renderBlocks(chunk: string): string {
  if (!chunk.trim()) return "";
  const lines = chunk.split("\n");
  const out: string[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim()) {
      i += 1;
      continue;
    }
    const heading = /^(#{1,4})\s+(.+)$/.exec(line);
    if (heading) {
      const level = heading[1].length;
      out.push(`<h${level}>${inlineFormat(escapeHtml(heading[2]))}</h${level}>`);
      i += 1;
      continue;
    }
    if (/^[-*]\s+/.test(line)) {
      const items: string[] = [];
      while (i < lines.length && /^[-*]\s+/.test(lines[i])) {
        items.push(`<li>${inlineFormat(escapeHtml(lines[i].replace(/^[-*]\s+/, "")))}</li>`);
        i += 1;
      }
      out.push(`<ul>${items.join("")}</ul>`);
      continue;
    }
    if (/^\d+\.\s+/.test(line)) {
      const items: string[] = [];
      while (i < lines.length && /^\d+\.\s+/.test(lines[i])) {
        items.push(`<li>${inlineFormat(escapeHtml(lines[i].replace(/^\d+\.\s+/, "")))}</li>`);
        i += 1;
      }
      out.push(`<ol>${items.join("")}</ol>`);
      continue;
    }
    const para: string[] = [line];
    i += 1;
    while (i < lines.length && lines[i].trim() && !/^(#{1,4}\s+|[-*]\s+|\d+\.\s+)/.test(lines[i])) {
      para.push(lines[i]);
      i += 1;
    }
    out.push(`<p>${inlineFormat(escapeHtml(para.join("\n"))).replace(/\n/g, "<br/>")}</p>`);
  }
  return out.join("");
}
