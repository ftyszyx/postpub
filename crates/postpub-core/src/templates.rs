use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

use chrono::{DateTime, Utc};
use include_dir::{include_dir, Dir};
use postpub_types::{
    CopyTemplateRequest, CreateTemplateCategoryRequest, CreateTemplateRequest, MoveTemplateRequest,
    RenameTemplateCategoryRequest, RenameTemplateRequest, TemplateCategorySummary,
    TemplateDocument, TemplateSummary, UpdateTemplateContentRequest,
};

use crate::{
    error::{PostpubError, Result},
    paths::AppPaths,
};

const DEFAULT_TEMPLATES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/default-templates");

#[derive(Debug, Clone)]
pub struct TemplateStore {
    paths: AppPaths,
}

impl TemplateStore {
    pub fn new(paths: AppPaths) -> Self {
        Self { paths }
    }

    pub fn ensure_defaults(&self) -> Result<()> {
        self.paths.ensure_directories()?;
        self.migrate_legacy_defaults()?;
        copy_embedded_templates(&DEFAULT_TEMPLATES, &self.paths.templates_dir())?;

        Ok(())
    }

    pub fn list_categories(&self) -> Result<Vec<TemplateCategorySummary>> {
        self.ensure_defaults()?;

        let mut categories = Vec::new();
        for entry in fs::read_dir(self.paths.templates_dir())? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                let count = fs::read_dir(entry.path())?
                    .filter_map(|item| item.ok())
                    .filter(|item| item.path().extension().is_some_and(|ext| ext == "html"))
                    .count();
                categories.push(TemplateCategorySummary {
                    name,
                    template_count: count,
                });
            }
        }

        categories.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(categories)
    }

    pub fn create_category(
        &self,
        request: &CreateTemplateCategoryRequest,
    ) -> Result<TemplateCategorySummary> {
        let name = sanitize_segment(&request.name)?;
        let path = self.paths.templates_dir().join(&name);
        if path.exists() {
            return Err(PostpubError::Conflict(format!(
                "template category already exists: {name}"
            )));
        }

        fs::create_dir_all(path)?;
        Ok(TemplateCategorySummary {
            name,
            template_count: 0,
        })
    }

    pub fn rename_category(
        &self,
        category_name: &str,
        request: &RenameTemplateCategoryRequest,
    ) -> Result<()> {
        let current = self
            .paths
            .templates_dir()
            .join(sanitize_segment(category_name)?);
        if !current.exists() {
            return Err(PostpubError::NotFound(format!(
                "template category not found: {category_name}"
            )));
        }

        let target_name = sanitize_segment(&request.new_name)?;
        let target = self.paths.templates_dir().join(&target_name);
        if target.exists() {
            return Err(PostpubError::Conflict(format!(
                "template category already exists: {target_name}"
            )));
        }

        fs::rename(current, target)?;
        Ok(())
    }

    pub fn delete_category(&self, category_name: &str) -> Result<()> {
        let path = self
            .paths
            .templates_dir()
            .join(sanitize_segment(category_name)?);
        if !path.exists() {
            return Err(PostpubError::NotFound(format!(
                "template category not found: {category_name}"
            )));
        }

        fs::remove_dir_all(path)?;
        Ok(())
    }

    pub fn list_templates(&self, category: Option<&str>) -> Result<Vec<TemplateSummary>> {
        self.ensure_defaults()?;

        let mut templates = Vec::new();
        if let Some(category_name) = category {
            let dir = self
                .paths
                .templates_dir()
                .join(sanitize_segment(category_name)?);
            self.collect_templates_from_dir(&dir, &mut templates)?;
        } else {
            for entry in fs::read_dir(self.paths.templates_dir())? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    self.collect_templates_from_dir(&entry.path(), &mut templates)?;
                }
            }
        }

        templates.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(templates)
    }

    pub fn get_template(&self, relative_path: &str) -> Result<TemplateDocument> {
        let path = self.resolve_template_path(relative_path)?;
        if !path.exists() {
            return Err(PostpubError::NotFound(format!(
                "template not found: {relative_path}"
            )));
        }

        let category = path
            .parent()
            .and_then(|parent| parent.file_name())
            .map(|name| name.to_string_lossy().to_string())
            .ok_or_else(|| PostpubError::InvalidPath(relative_path.to_string()))?;

        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .ok_or_else(|| PostpubError::InvalidPath(relative_path.to_string()))?;

        Ok(TemplateDocument {
            name,
            category,
            relative_path: normalize_relative_path(relative_path)?,
            content: fs::read_to_string(path)?,
        })
    }

    pub fn create_template(&self, request: &CreateTemplateRequest) -> Result<TemplateDocument> {
        let category = sanitize_segment(&request.category)?;
        let name = sanitize_segment(&request.name)?;
        let dir = self.paths.templates_dir().join(&category);
        fs::create_dir_all(&dir)?;

        let path = dir.join(format!("{name}.html"));
        if path.exists() {
            return Err(PostpubError::Conflict(format!(
                "template already exists: {category}/{name}.html"
            )));
        }

        fs::write(&path, &request.content)?;
        self.get_template(&relative_from_templates_root(
            &self.paths.templates_dir(),
            &path,
        )?)
    }

    pub fn update_template(
        &self,
        relative_path: &str,
        request: &UpdateTemplateContentRequest,
    ) -> Result<TemplateDocument> {
        let path = self.resolve_template_path(relative_path)?;
        if !path.exists() {
            return Err(PostpubError::NotFound(format!(
                "template not found: {relative_path}"
            )));
        }

        fs::write(path, &request.content)?;
        self.get_template(relative_path)
    }

    pub fn delete_template(&self, relative_path: &str) -> Result<()> {
        let path = self.resolve_template_path(relative_path)?;
        if !path.exists() {
            return Err(PostpubError::NotFound(format!(
                "template not found: {relative_path}"
            )));
        }

        fs::remove_file(path)?;
        Ok(())
    }

    pub fn rename_template(&self, request: &RenameTemplateRequest) -> Result<TemplateDocument> {
        let current_path = self.resolve_template_path(&request.relative_path)?;
        if !current_path.exists() {
            return Err(PostpubError::NotFound(format!(
                "template not found: {}",
                request.relative_path
            )));
        }

        let new_name = sanitize_segment(&request.new_name)?;
        let target = current_path
            .parent()
            .unwrap_or(&self.paths.templates_dir())
            .join(format!("{new_name}.html"));
        if target.exists() {
            return Err(PostpubError::Conflict(format!(
                "template already exists: {}",
                target.display()
            )));
        }

        fs::rename(&current_path, &target)?;
        self.get_template(&relative_from_templates_root(
            &self.paths.templates_dir(),
            &target,
        )?)
    }

    pub fn copy_template(&self, request: &CopyTemplateRequest) -> Result<TemplateDocument> {
        let source = self.resolve_template_path(&request.relative_path)?;
        if !source.exists() {
            return Err(PostpubError::NotFound(format!(
                "template not found: {}",
                request.relative_path
            )));
        }

        let target_category = sanitize_segment(&request.target_category)?;
        let new_name = sanitize_segment(&request.new_name)?;
        let target_dir = self.paths.templates_dir().join(&target_category);
        fs::create_dir_all(&target_dir)?;
        let target = target_dir.join(format!("{new_name}.html"));
        if target.exists() {
            return Err(PostpubError::Conflict(format!(
                "template already exists: {target_category}/{new_name}.html"
            )));
        }

        fs::copy(&source, &target)?;
        self.get_template(&relative_from_templates_root(
            &self.paths.templates_dir(),
            &target,
        )?)
    }

    pub fn move_template(&self, request: &MoveTemplateRequest) -> Result<TemplateDocument> {
        let source = self.resolve_template_path(&request.relative_path)?;
        if !source.exists() {
            return Err(PostpubError::NotFound(format!(
                "template not found: {}",
                request.relative_path
            )));
        }

        let target_category = sanitize_segment(&request.target_category)?;
        let target_dir = self.paths.templates_dir().join(&target_category);
        fs::create_dir_all(&target_dir)?;
        let target = target_dir.join(
            source
                .file_name()
                .ok_or_else(|| PostpubError::InvalidPath(request.relative_path.clone()))?,
        );
        fs::rename(&source, &target)?;
        self.get_template(&relative_from_templates_root(
            &self.paths.templates_dir(),
            &target,
        )?)
    }

    pub fn load_selected_template(
        &self,
        category: Option<&str>,
        name: Option<&str>,
    ) -> Result<Option<TemplateDocument>> {
        let Some(category) = category.filter(|value| !value.trim().is_empty()) else {
            return Ok(None);
        };
        let Some(name) = name.filter(|value| !value.trim().is_empty()) else {
            return Ok(None);
        };

        let relative = format!(
            "{}/{}.html",
            sanitize_segment(category)?,
            sanitize_segment(name)?
        );
        if self.resolve_template_path(&relative)?.exists() {
            return self.get_template(&relative).map(Some);
        }

        Ok(None)
    }

    fn collect_templates_from_dir(
        &self,
        dir: &Path,
        templates: &mut Vec<TemplateSummary>,
    ) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        let category = dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .ok_or_else(|| PostpubError::InvalidPath(dir.display().to_string()))?;

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !entry.file_type()?.is_file() || path.extension().is_none_or(|ext| ext != "html") {
                continue;
            }

            let metadata = fs::metadata(&path)?;
            let updated_at: DateTime<Utc> =
                DateTime::<Utc>::from(metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH));
            templates.push(TemplateSummary {
                name: path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().to_string())
                    .unwrap_or_else(|| "template".to_string()),
                category: category.clone(),
                relative_path: relative_from_templates_root(&self.paths.templates_dir(), &path)?,
                size_bytes: metadata.len(),
                updated_at,
            });
        }

        Ok(())
    }

    fn resolve_template_path(&self, relative_path: &str) -> Result<PathBuf> {
        let relative = relative_components(relative_path)?;
        Ok(self.paths.templates_dir().join(relative))
    }

    fn migrate_legacy_defaults(&self) -> Result<()> {
        let templates_root = self.paths.templates_dir();
        let general_dir = templates_root.join("general");
        if !general_dir.exists() {
            return Ok(());
        }

        let categories = fs::read_dir(&templates_root)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                entry
                    .file_type()
                    .ok()
                    .filter(|kind| kind.is_dir())
                    .map(|_| entry)
            })
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect::<Vec<_>>();

        if categories.len() != 1 || categories.first().is_none_or(|value| value != "general") {
            return Ok(());
        }

        let mut html_files = fs::read_dir(&general_dir)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                entry
                    .file_type()
                    .ok()
                    .filter(|kind| kind.is_file())
                    .and_then(|_| {
                        entry
                            .path()
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                    })
            })
            .collect::<Vec<_>>();
        html_files.sort();

        if html_files == ["magazine.html".to_string(), "minimal.html".to_string()] {
            fs::remove_dir_all(general_dir)?;
        }

        Ok(())
    }
}

fn copy_embedded_templates(dir: &Dir<'_>, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;

    for entry in dir.entries() {
        if let Some(subdir) = entry.as_dir() {
            let name = subdir
                .path()
                .file_name()
                .ok_or_else(|| PostpubError::InvalidPath(subdir.path().display().to_string()))?;
            copy_embedded_templates(subdir, &destination.join(name))?;
            continue;
        }

        let Some(file) = entry.as_file() else {
            continue;
        };
        let name = file
            .path()
            .file_name()
            .ok_or_else(|| PostpubError::InvalidPath(file.path().display().to_string()))?;
        let output = destination.join(name);
        if !output.exists() {
            fs::write(output, file.contents())?;
        }
    }

    Ok(())
}

fn sanitize_segment(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(PostpubError::Validation("name cannot be empty".to_string()));
    }

    if trimmed
        .chars()
        .any(|ch| matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
    {
        return Err(PostpubError::Validation(format!(
            "invalid path segment: {trimmed}"
        )));
    }

    Ok(trimmed.to_string())
}

fn normalize_relative_path(value: &str) -> Result<String> {
    let relative = relative_components(value)?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn relative_components(value: &str) -> Result<PathBuf> {
    let candidate = PathBuf::from(value);
    if candidate.is_absolute() {
        return Err(PostpubError::InvalidPath(value.to_string()));
    }

    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(PostpubError::InvalidPath(value.to_string()));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(PostpubError::InvalidPath(value.to_string()));
    }

    Ok(normalized)
}

fn relative_from_templates_root(root: &Path, path: &Path) -> Result<String> {
    Ok(path
        .strip_prefix(root)
        .map_err(|_| PostpubError::InvalidPath(path.display().to_string()))?
        .to_string_lossy()
        .replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::TemplateStore;
    use crate::paths::AppPaths;
    use postpub_types::{
        CreateTemplateRequest, MoveTemplateRequest, RenameTemplateCategoryRequest,
        RenameTemplateRequest, UpdateTemplateContentRequest,
    };

    #[test]
    fn manages_templates_in_workspace() {
        let temp = tempdir().expect("temp dir");
        let store = TemplateStore::new(AppPaths::from_root(temp.path().to_path_buf()));
        store.ensure_defaults().expect("defaults");

        store
            .create_template(&CreateTemplateRequest {
                name: "note".to_string(),
                category: "custom".to_string(),
                content: "<h1>Hello</h1>".to_string(),
            })
            .expect("create");

        let mut templates = store.list_templates(Some("custom")).expect("list");
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].relative_path, "custom/note.html");

        store
            .update_template(
                "custom/note.html",
                &UpdateTemplateContentRequest {
                    content: "<h1>Updated</h1>".to_string(),
                },
            )
            .expect("update");

        let renamed = store
            .rename_template(&RenameTemplateRequest {
                relative_path: "custom/note.html".to_string(),
                new_name: "note-v2".to_string(),
            })
            .expect("rename");
        assert_eq!(renamed.relative_path, "custom/note-v2.html");

        store
            .rename_category(
                "custom",
                &RenameTemplateCategoryRequest {
                    new_name: "renamed".to_string(),
                },
            )
            .expect("rename category");

        templates = store.list_templates(Some("renamed")).expect("list renamed");
        assert_eq!(templates.len(), 1);

        let moved = store
            .move_template(&MoveTemplateRequest {
                relative_path: "renamed/note-v2.html".to_string(),
                target_category: "archive".to_string(),
            })
            .expect("move");
        assert_eq!(moved.relative_path, "archive/note-v2.html");
    }

    #[test]
    fn seeds_aiforge_builtin_templates() {
        let temp = tempdir().expect("temp dir");
        let store = TemplateStore::new(AppPaths::from_root(temp.path().to_path_buf()));

        store.ensure_defaults().expect("defaults");

        let templates = store.list_templates(None).expect("list defaults");
        assert_eq!(templates.len(), 35);
        assert!(templates
            .iter()
            .any(|item| item.relative_path == "其他/template1.html"));
        assert!(templates
            .iter()
            .any(|item| item.relative_path == "健康养生/t1.html"));
    }
}
