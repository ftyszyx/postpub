import { describe, expect, it } from "vitest";
import { buildTemplatePreviewHtml } from "./template-preview";

const sampleData = {
  title: "Sample title",
  summary: "Sample summary",
  paragraphOne: "First paragraph",
  paragraphTwo: "Second paragraph",
  generatedAt: "2026-03-30 22:00",
  author: "postpub",
  platform: "WeChat"
};

describe("buildTemplatePreviewHtml", () => {
  it("renders html preview content by default", () => {
    const preview = buildTemplatePreviewHtml("<main>{{content}}</main>", sampleData);

    expect(preview).toContain('<p style="white-space: pre-wrap;">First paragraph</p>');
    expect(preview).toContain('<p style="white-space: pre-wrap;">Second paragraph</p>');
    expect(preview).not.toContain("<blockquote>");
  });

  it("renders markdown source content when markdown mode is selected", () => {
    const preview = buildTemplatePreviewHtml("## {{title}}\n\n- {{summary}}\n\n> {{paragraphTwo}}", sampleData, "markdown");

    expect(preview).toContain("<h2>Sample title</h2>");
    expect(preview).toContain("<li>Sample summary</li>");
    expect(preview).toContain("<blockquote>");
    expect(preview).toContain("<p>Second paragraph</p>");
  });

  it("escapes placeholder content in text preview", () => {
    const preview = buildTemplatePreviewHtml("{{title}}\n{{paragraphOne}}", {
      ...sampleData,
      title: "<b>Unsafe</b>",
      paragraphOne: "<script>alert(1)</script>"
    }, "text");

    expect(preview).toContain("&lt;b&gt;Unsafe&lt;/b&gt;");
    expect(preview).toContain("&lt;script&gt;alert(1)&lt;/script&gt;");
    expect(preview).not.toContain("<script>alert(1)</script>");
  });
});
