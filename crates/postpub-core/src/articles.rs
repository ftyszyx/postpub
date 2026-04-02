use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

use chrono::{DateTime, Utc};
use postpub_types::{ArticleDesign, ArticleDocument, ArticleSummary, UpdateArticleContentRequest};
use regex::Regex;

use crate::{
    error::{PostpubError, Result},
    paths::AppPaths,
};

#[derive(Debug, Clone)]
pub struct ArticleStore {
    paths: AppPaths,
}

impl ArticleStore {
    pub fn new(paths: AppPaths) -> Self {
        Self { paths }
    }

    pub fn ensure_defaults(&self) -> Result<()> {
        self.paths.ensure_directories()?;
        if !self.paths.publish_records_file().exists() {
            fs::write(
                self.paths.publish_records_file(),
                serde_json::to_string_pretty(&serde_json::json!({}))?,
            )?;
        }
        Ok(())
    }

    pub fn list_articles(&self) -> Result<Vec<ArticleSummary>> {
        self.ensure_defaults()?;

        let mut articles = Vec::new();
        for entry in fs::read_dir(self.paths.articles_dir())? {
            let entry = entry?;
            let path = entry.path();
            if !entry.file_type()?.is_file() {
                continue;
            }

            let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
                continue;
            };
            if !matches!(extension, "html" | "md" | "txt") {
                continue;
            }

            articles.push(self.summary_from_path(&path)?);
        }

        articles.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(articles)
    }

    pub fn get_article(&self, relative_path: &str) -> Result<ArticleDocument> {
        let path = self.resolve_article_path(relative_path)?;
        if !path.exists() {
            return Err(PostpubError::NotFound(format!(
                "article not found: {relative_path}"
            )));
        }

        let summary = self.summary_from_path(&path)?;
        let content = fs::read_to_string(&path)?;
        let preview_html = preview_html(summary.format.as_str(), &content);

        Ok(ArticleDocument {
            summary,
            content,
            preview_html,
        })
    }

    pub fn update_article(
        &self,
        relative_path: &str,
        request: &UpdateArticleContentRequest,
    ) -> Result<ArticleDocument> {
        let path = self.resolve_article_path(relative_path)?;
        if !path.exists() {
            return Err(PostpubError::NotFound(format!(
                "article not found: {relative_path}"
            )));
        }

        fs::write(&path, &request.content)?;
        self.get_article(relative_path)
    }

    pub fn delete_article(&self, relative_path: &str) -> Result<()> {
        let path = self.resolve_article_path(relative_path)?;
        if !path.exists() {
            return Err(PostpubError::NotFound(format!(
                "article not found: {relative_path}"
            )));
        }

        fs::remove_file(&path)?;
        let design_path = article_design_path(&path)?;
        if design_path.exists() {
            fs::remove_file(design_path)?;
        }
        Ok(())
    }

    pub fn save_generated_article(
        &self,
        title: &str,
        format: &str,
        content: &str,
    ) -> Result<ArticleDocument> {
        self.ensure_defaults()?;

        let normalized_format = normalize_format(format);
        let file_name = format!(
            "{}.{}",
            sanitize_filename(title),
            normalized_format.as_str()
        );
        let path = self.paths.articles_dir().join(&file_name);
        fs::write(&path, content)?;

        self.get_article(&file_name.replace('\\', "/"))
    }

    pub fn load_article_design(&self, relative_path: &str) -> Result<ArticleDesign> {
        let path = self.resolve_article_path(relative_path)?;
        if !path.exists() {
            return Err(PostpubError::NotFound(format!(
                "article not found: {relative_path}"
            )));
        }

        let design_path = article_design_path(&path)?;
        if !design_path.exists() {
            return Ok(ArticleDesign::default());
        }

        Ok(serde_json::from_str(&fs::read_to_string(design_path)?)?)
    }

    pub fn save_article_design(
        &self,
        relative_path: &str,
        design: &ArticleDesign,
    ) -> Result<ArticleDesign> {
        let path = self.resolve_article_path(relative_path)?;
        if !path.exists() {
            return Err(PostpubError::NotFound(format!(
                "article not found: {relative_path}"
            )));
        }

        let design_path = article_design_path(&path)?;
        fs::write(design_path, serde_json::to_string_pretty(design)?)?;
        Ok(design.clone())
    }

    fn summary_from_path(&self, path: &Path) -> Result<ArticleSummary> {
        let metadata = fs::metadata(path)?;
        let updated_at: DateTime<Utc> =
            DateTime::<Utc>::from(metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH));
        let relative_path = path
            .strip_prefix(self.paths.articles_dir())
            .map_err(|_| PostpubError::InvalidPath(path.display().to_string()))?
            .to_string_lossy()
            .replace('\\', "/");

        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .ok_or_else(|| PostpubError::InvalidPath(path.display().to_string()))?;
        let format = path
            .extension()
            .map(|ext| ext.to_string_lossy().to_uppercase())
            .unwrap_or_else(|| "TXT".to_string());

        Ok(ArticleSummary {
            title: name.replace('_', "|"),
            name,
            relative_path,
            format,
            size_bytes: metadata.len(),
            updated_at,
            status: "draft".to_string(),
        })
    }

    fn resolve_article_path(&self, relative_path: &str) -> Result<PathBuf> {
        let candidate = PathBuf::from(relative_path);
        if candidate.is_absolute() {
            return Err(PostpubError::InvalidPath(relative_path.to_string()));
        }

        let mut normalized = PathBuf::new();
        for component in candidate.components() {
            match component {
                Component::Normal(value) => normalized.push(value),
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(PostpubError::InvalidPath(relative_path.to_string()));
                }
            }
        }

        Ok(self.paths.articles_dir().join(normalized))
    }
}

pub fn preview_html(format: &str, content: &str) -> String {
    match normalize_format(format).as_str() {
        "html" => content.to_string(),
        "md" => markdown_to_html(content),
        _ => text_to_html(content),
    }
}

pub fn markdown_to_html(markdown: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(markdown, options);
    let mut rendered = String::new();
    html::push_html(&mut rendered, parser);
    rendered
}

fn text_to_html(text: &str) -> String {
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        "<article><pre style=\"white-space: pre-wrap; font-family: inherit;\">{escaped}</pre></article>"
    )
}

fn sanitize_filename(title: &str) -> String {
    let invalid = Regex::new(r#"[^\p{L}\p{N}\-_]+"#).expect("regex");
    let normalized = title.replace('|', "_");
    let collapsed = invalid.replace_all(&normalized, "_");
    let trimmed = collapsed.trim_matches('_');
    if trimmed.is_empty() {
        "postpub_article".to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_format(format: &str) -> String {
    match format.trim().to_ascii_lowercase().as_str() {
        "html" => "html".to_string(),
        "md" | "markdown" => "md".to_string(),
        _ => "txt".to_string(),
    }
}

fn article_design_path(article_path: &Path) -> Result<PathBuf> {
    let Some(parent) = article_path.parent() else {
        return Err(PostpubError::InvalidPath(article_path.display().to_string()));
    };
    let Some(stem) = article_path.file_stem() else {
        return Err(PostpubError::InvalidPath(article_path.display().to_string()));
    };

    Ok(parent.join(format!("{}.design.json", stem.to_string_lossy())))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{markdown_to_html, ArticleStore};
    use crate::paths::AppPaths;

    #[test]
    fn saves_and_reads_generated_articles() {
        let temp = tempdir().expect("temp dir");
        let store = ArticleStore::new(AppPaths::from_root(temp.path().to_path_buf()));

        let article = store
            .save_generated_article("Platform|Topic", "md", "# Title\n\nContent")
            .expect("save article");
        assert_eq!(article.summary.relative_path, "Platform_Topic.md");

        let list = store.list_articles().expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].format, "MD");
        assert!(markdown_to_html("# Heading").contains("<h1>Heading</h1>"));
    }

    #[test]
    fn persists_article_design_next_to_article() {
        let temp = tempdir().expect("temp dir");
        let store = ArticleStore::new(AppPaths::from_root(temp.path().to_path_buf()));

        let article = store
            .save_generated_article("Design|Demo", "html", "<h1>Design</h1>")
            .expect("save article");

        let saved = store
            .save_article_design(
                &article.summary.relative_path,
                &postpub_types::ArticleDesign {
                    html: "<section>draft</section>".to_string(),
                    css: "section { color: red; }".to_string(),
                    cover: "/images/demo.png".to_string(),
                },
            )
            .expect("save design");
        assert_eq!(saved.cover, "/images/demo.png");

        let loaded = store
            .load_article_design(&article.summary.relative_path)
            .expect("load design");
        assert_eq!(loaded.html, "<section>draft</section>");

        store
            .delete_article(&article.summary.relative_path)
            .expect("delete article");
        assert!(store
            .load_article_design(&article.summary.relative_path)
            .is_err());
    }
}
