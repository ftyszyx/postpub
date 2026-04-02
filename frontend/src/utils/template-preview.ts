import { marked } from "marked";

export type TemplatePreviewMode = "html" | "markdown" | "text";

export interface TemplatePreviewSampleData {
  title: string;
  summary: string;
  paragraphOne: string;
  paragraphTwo: string;
  generatedAt: string;
  author: string;
  platform: string;
}

function escapeHtml(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function buildPreviewContentHtml(sampleData: TemplatePreviewSampleData) {
  return [escapeHtml(sampleData.paragraphOne), escapeHtml(sampleData.paragraphTwo)]
    .map((paragraph) => `<p style="white-space: pre-wrap;">${paragraph}</p>`)
    .join("");
}

function buildPreviewContentMarkdown(sampleData: TemplatePreviewSampleData) {
  return [
    `## ${escapeHtml(sampleData.title)}`,
    "",
    escapeHtml(sampleData.paragraphOne),
    "",
    `- ${escapeHtml(sampleData.summary)}`,
    `- ${escapeHtml(sampleData.platform)}`,
    "",
    `> ${escapeHtml(sampleData.paragraphTwo)}`
  ].join("\n");
}

function buildPreviewContentText(sampleData: TemplatePreviewSampleData) {
  return [sampleData.paragraphOne, "", sampleData.paragraphTwo].join("\n");
}

function buildReplacementMap(sampleData: TemplatePreviewSampleData, mode: TemplatePreviewMode) {
  const plainTitle = mode === "text" ? sampleData.title : escapeHtml(sampleData.title);
  const plainSummary = mode === "text" ? sampleData.summary : escapeHtml(sampleData.summary);
  const plainParagraphOne = mode === "text" ? sampleData.paragraphOne : escapeHtml(sampleData.paragraphOne);
  const plainParagraphTwo = mode === "text" ? sampleData.paragraphTwo : escapeHtml(sampleData.paragraphTwo);
  const plainGeneratedAt = mode === "text" ? sampleData.generatedAt : escapeHtml(sampleData.generatedAt);
  const plainAuthor = mode === "text" ? sampleData.author : escapeHtml(sampleData.author);
  const plainPlatform = mode === "text" ? sampleData.platform : escapeHtml(sampleData.platform);

  return {
    title: plainTitle,
    summary: plainSummary,
    content:
      mode === "html"
        ? buildPreviewContentHtml(sampleData)
        : mode === "markdown"
          ? buildPreviewContentMarkdown(sampleData)
          : buildPreviewContentText(sampleData),
    paragraphOne: plainParagraphOne,
    paragraphTwo: plainParagraphTwo,
    generated_at: plainGeneratedAt,
    author: plainAuthor,
    platform: plainPlatform
  };
}

function wrapPreviewDocument(bodyContent: string, extraStyles = "") {
  return [
    "<!DOCTYPE html>",
    '<html lang="zh-CN">',
    "  <head>",
    '    <meta charset="UTF-8" />',
    '    <meta name="viewport" content="width=device-width, initial-scale=1.0" />',
    "    <style>",
    "      body { margin: 0; padding: 8px; overflow: auto; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', sans-serif; line-height: 1.45; font-size: 12px; color: #1f2937; background: #ffffff; }",
    "      h1, h2, h3, h4, h5, h6 { margin: 4px 0 6px; font-weight: 700; }",
    "      h1 { font-size: 16px; }",
    "      h2 { font-size: 14px; }",
    "      h3 { font-size: 13px; }",
    "      p { margin: 0 0 6px; }",
    "      ul, ol { margin: 0 0 8px; padding-left: 18px; }",
    "      li { margin-bottom: 4px; }",
    "      img { max-width: 100%; height: auto; display: block; }",
    "      blockquote { margin: 4px 0; padding: 4px 8px; border-left: 3px solid #d1d5db; background: #f8fafc; }",
    "      code { background: #f3f4f6; padding: 1px 4px; border-radius: 3px; }",
    "      pre { background: #f3f4f6; padding: 6px; border-radius: 4px; overflow: auto; }",
    "      table { width: 100%; border-collapse: collapse; font-size: 10px; }",
    "      th, td { border: 1px solid #e5e7eb; padding: 2px 4px; }",
    "      ::-webkit-scrollbar { width: 8px; height: 8px; }",
    "      ::-webkit-scrollbar-thumb { background: #cbd5e1; border-radius: 999px; }",
    "      * { scrollbar-width: thin; scrollbar-color: #cbd5e1 transparent; }",
    extraStyles,
    "    </style>",
    "  </head>",
    "  <body>",
    bodyContent,
    "  </body>",
    "</html>"
  ].join("\n");
}

function renderTextPreview(content: string) {
  return content
    .split("\n")
    .map((line) => (line.trim() ? `<p style="white-space: pre-wrap;">${escapeHtml(line)}</p>` : "<br />"))
    .join("\n");
}

function renderMarkdownPreview(content: string) {
  marked.setOptions({
    gfm: true,
    breaks: true,
    async: false
  });

  return marked.parse(content) as string;
}

export function buildTemplatePreviewHtml(
  content: string,
  sampleData: TemplatePreviewSampleData,
  mode: TemplatePreviewMode = "html"
) {
  const replacementMap = buildReplacementMap(sampleData, mode);
  const rendered = content.replace(/\{\{\s*([a-zA-Z0-9_]+)\s*\}\}/g, (source, key: string) => {
    return replacementMap[key as keyof typeof replacementMap] ?? source;
  });

  if (mode === "markdown") {
    return wrapPreviewDocument(
      renderMarkdownPreview(rendered),
      "      body { padding: 16px; line-height: 1.6; }\n      blockquote p { margin-bottom: 0; }\n      hr { border: 0; border-top: 1px solid #e5e7eb; margin: 16px 0; }"
    );
  }

  if (mode === "text") {
    return wrapPreviewDocument(
      renderTextPreview(rendered),
      "      body { padding: 16px; }\n      br { display: block; content: ''; margin-bottom: 8px; }"
    );
  }

  if (/<html[\s>]/i.test(rendered)) return rendered;

  return wrapPreviewDocument(rendered, "      body { overflow: hidden; }");
}
