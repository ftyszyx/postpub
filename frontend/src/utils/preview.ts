export type PreviewDesignSurface = "light" | "dark";

function themedPreviewStyle(designSurface: PreviewDesignSurface) {
  if (designSurface === "dark") {
    return [
      ":root { color-scheme: dark; }",
      "html, body { margin: 0; background: #0b1120; color: #e2e8f0; }",
      "body { font-family: \"Segoe UI\", \"PingFang SC\", \"Microsoft YaHei\", sans-serif; }",
      "a { color: #93c5fd; }",
      "img, video { max-width: 100%; }",
      "pre, code { background: rgba(148, 163, 184, 0.12); color: #e2e8f0; }",
      "blockquote { border-left: 3px solid #334155; color: #cbd5e1; margin-left: 0; padding-left: 1rem; }",
      "table { border-color: #334155; }"
    ].join("");
  }

  return [
    ":root { color-scheme: light; }",
    "html, body { margin: 0; background: #ffffff; color: #0f172a; }",
    "body { font-family: \"Segoe UI\", \"PingFang SC\", \"Microsoft YaHei\", sans-serif; }",
    "a { color: #2563eb; }",
    "img, video { max-width: 100%; }"
  ].join("");
}

export function decoratePreviewDocument(
  documentHtml: string,
  designSurface: PreviewDesignSurface,
  title = "Preview"
) {
  const source = documentHtml?.trim();
  if (!source) {
    return "";
  }

  const styleTag = `<style data-postpub-preview-theme>${themedPreviewStyle(designSurface)}</style>`;

  if (/<html[\s>]/i.test(source)) {
    if (/<head[\s>]/i.test(source)) {
      return source.replace(/<head([^>]*)>/i, `<head$1>${styleTag}`);
    }

    return source.replace(/<html([^>]*)>/i, `<html$1><head>${styleTag}</head>`);
  }

  return `<!DOCTYPE html><html lang="en"><head><meta charset="utf-8" /><meta name="viewport" content="width=device-width, initial-scale=1" /><title>${title}</title>${styleTag}</head><body>${source}</body></html>`;
}
