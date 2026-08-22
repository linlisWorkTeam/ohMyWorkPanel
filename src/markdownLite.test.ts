import { describe, expect, it } from "vitest";
import { markdownToHtml } from "./markdownLite";

describe("markdownLite", () => {
  it("renders headings, bold, code, and lists", () => {
    const html = markdownToHtml("# Title\n\nHello **world** and `x`\n\n- a\n- b");
    expect(html).toContain("<h1>");
    expect(html).toContain("<strong>world</strong>");
    expect(html).toContain("<code>x</code>");
    expect(html).toContain("<ul>");
    expect(html).toContain("<li>");
  });

  it("escapes raw HTML", () => {
    const html = markdownToHtml("a <script>alert(1)</script>");
    expect(html).not.toContain("<script>");
    expect(html).toContain("&lt;script&gt;");
  });

  it("renders fenced code blocks", () => {
    const html = markdownToHtml("```js\nconst a = 1;\n```");
    expect(html).toContain('<pre class="md-code"');
    expect(html).toContain("const a = 1;");
  });
});
