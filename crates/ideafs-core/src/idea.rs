use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};

use jiff::Zoned;

use crate::{
    category::{
        category_directories, category_status, ensure_default_categories, resolve_category_query,
    },
    models::{CliIdea, FileData, IdeaFile, IdeaFolder, MetaData, TreeData},
    paths::data_root,
};

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn sanitize_filename(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|character| {
            if character.is_control() || r#"\/:*?"<>|"#.contains(character) {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches([' ', '.']);
    if sanitized.is_empty() {
        "untitled".to_owned()
    } else {
        sanitized.to_owned()
    }
}

pub fn idea_path(root: &Path, filepath: &str) -> Result<PathBuf, String> {
    let relative_path = Path::new(filepath);
    let mut components = relative_path.components();
    let status_dir = components
        .next()
        .filter(|component| matches!(component, Component::Normal(_)))
        .and_then(|component| component.as_os_str().to_str())
        .ok_or_else(|| "Invalid idea file path".to_owned())?;
    let filename = components
        .next()
        .filter(|component| matches!(component, Component::Normal(_)))
        .and_then(|component| component.as_os_str().to_str())
        .ok_or_else(|| "Idea paths must point to a Markdown file in a status folder".to_owned())?;
    if components.next().is_some()
        || Path::new(filename)
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("md")
    {
        return Err("Invalid idea file path".to_owned());
    }

    let path = root.join(relative_path);
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("Failed to access idea data directory: {error}"))?;
    let category_path = root.join(status_dir);
    let canonical_category = category_path.canonicalize().map_err(|error| {
        format!(
            "Failed to access category {}: {error}",
            category_path.display()
        )
    })?;

    if !canonical_category.starts_with(&canonical_root) || !canonical_category.is_dir() {
        return Err("Idea category is outside the data directory".to_owned());
    }
    let canonical_path = path
        .canonicalize()
        .map_err(|error| format!("Failed to access {}: {error}", path.display()))?;
        
    if !canonical_path.starts_with(canonical_root) || !canonical_path.is_file() {
        return Err("Idea file path is outside the data directory".to_owned());
    }

    Ok(canonical_path)
}

pub fn parse_file(filepath: String, path: &Path) -> Result<FileData, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    let frontmatter = raw
        .strip_prefix("---\n")
        .or_else(|| raw.strip_prefix("---\r\n"))
        .ok_or_else(|| format!("{} has no YAML frontmatter", path.display()))?;
    let (yaml, content) = frontmatter
        .split_once("\n---")
        .ok_or_else(|| format!("{} has an unterminated YAML frontmatter", path.display()))?;
    let metadata: MetaData = serde_yaml::from_str(yaml)
        .map_err(|error| format!("Invalid YAML in {}: {error}", path.display()))?;
    let content = content
        .strip_prefix("\r\n")
        .or_else(|| content.strip_prefix('\n'))
        .unwrap_or(content)
        .to_owned();

    Ok(FileData {
        filepath,
        metadata,
        content,
    })
}

pub fn create_idea_in_directory(
    root: &Path,
    directory: &str,
    title: &str,
    tags: Vec<String>,
    content: String,
    icon: Option<String>,
    resource_path: Option<String>,
    app_type: Option<String>,
    url: Option<String>,
) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Idea title cannot be empty".to_owned());
    }

    let status = category_status(directory);
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("Failed to access idea data directory: {error}"))?;
    let folder = root.join(directory);
    fs::create_dir_all(&folder)
        .map_err(|error| format!("Failed to create {}: {error}", folder.display()))?;
    let canonical_folder = folder
        .canonicalize()
        .map_err(|error| format!("Failed to access {}: {error}", folder.display()))?;
    if !canonical_folder.starts_with(&canonical_root) {
        return Err("Idea status folder is outside the data directory".to_owned());
    }

    let now = Zoned::now();
    let created_at = now.strftime("%+").to_string();
    let timestamp_stem = now.strftime("%Y%m%d%H%M%S").to_string();
    let metadata = MetaData {
        title: title.to_owned(),
        tags: tags
            .into_iter()
            .map(|tag| tag.trim().to_owned())
            .filter(|tag| !tag.is_empty())
            .collect(),
        status,
        created_at,
        icon: non_empty(icon),
        resource_path: non_empty(resource_path),
        app_type: non_empty(app_type),
        url: non_empty(url),
    };
    let yaml = serde_yaml::to_string(&metadata)
        .map_err(|error| format!("Failed to serialize idea metadata: {error}"))?;
    let markdown = format!("---\n{yaml}---\n\n{content}");
    let stem = format!("{timestamp_stem}_{}", sanitize_filename(title));

    for suffix in 0.. {
        let filename = if suffix == 0 {
            format!("{stem}.md")
        } else {
            format!("{stem}_{suffix}.md")
        };
        let path = canonical_folder.join(&filename);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(markdown.as_bytes()) {
                    drop(file);
                    return match fs::remove_file(&path) {
                        Ok(()) => Err(format!("Failed to write {}: {error}", path.display())),
                        Err(cleanup_error) => Err(format!(
                            "Failed to write {}: {error}; cleanup also failed: {cleanup_error}",
                            path.display()
                        )),
                    };
                }
                return Ok(format!("{directory}/{filename}"));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!("Failed to create {}: {error}", path.display()));
            }
        }
    }

    unreachable!("the filename suffix range is unbounded")
}

pub fn collect_idea_tree(root: &Path) -> Result<TreeData, String> {
    fs::create_dir_all(root)
        .map_err(|error| format!("Failed to create {}: {error}", root.display()))?;
    ensure_default_categories(root)?;
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("Failed to access idea data directory: {error}"))?;
    let categories = category_directories(root)?;
    let mut folders = Vec::with_capacity(categories.len());
    for directory in categories {
        let folder_path = root.join(&directory);
        let canonical_folder = folder_path
            .canonicalize()
            .map_err(|error| format!("Failed to access {}: {error}", folder_path.display()))?;
        if !canonical_folder.starts_with(&canonical_root) {
            return Err(format!(
                "Status folder is outside the data directory: {}",
                folder_path.display()
            ));
        }
        let entries = fs::read_dir(&folder_path)
            .map_err(|error| format!("Failed to list {}: {error}", folder_path.display()))?;
        let mut files = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!(
                    "Failed to read an entry in {}: {error}",
                    folder_path.display()
                )
            })?;
            let path = entry.path();
            if path.is_file()
                && path.extension().and_then(|extension| extension.to_str()) == Some("md")
            {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|error| format!("Invalid idea file path: {error}"))?
                    .to_string_lossy()
                    .replace('\\', "/");
                let file = parse_file(relative, &path)?;
                files.push(IdeaFile {
                    filepath: file.filepath,
                    metadata: file.metadata,
                });
            }
        }
        files.sort_by(|left, right| {
            left.metadata
                .title
                .to_lowercase()
                .cmp(&right.metadata.title.to_lowercase())
        });
        folders.push(IdeaFolder {
            name: directory.clone(),
            status: category_status(&directory),
            files,
        });
    }
    Ok(TreeData { folders })
}

pub fn resolve_status_directory(root: &Path, status: &str) -> Result<String, String> {
    let categories = category_directories(root)?;
    categories
        .iter()
        .find(|directory| category_status(directory) == status)
        .cloned()
        .ok_or_else(|| "Invalid idea status".to_owned())
}

// ——— CLI (no Tauri handle) ———

pub fn cli_add_idea(
    title: &str,
    category: Option<&str>,
    tags: Vec<String>,
    resource_path: Option<String>,
    url: Option<String>,
) -> Result<String, String> {
    let root = data_root()?;
    fs::create_dir_all(&root)
        .map_err(|error| format!("Failed to create {}: {error}", root.display()))?;
    ensure_default_categories(&root)?;
    let categories = category_directories(&root)?;
    let directory = if let Some(query) = category {
        resolve_category_query(&categories, query)?
    } else {
        categories
            .iter()
            .find(|directory| category_status(directory) == "inbox")
            .ok_or_else(|| "Inbox category is unavailable".to_owned())?
    };
    create_idea_in_directory(
        &root,
        directory,
        title,
        tags,
        String::new(),
        None,
        resource_path,
        None,
        url,
    )
}

pub fn cli_list_ideas(category: Option<&str>) -> Result<Vec<CliIdea>, String> {
    let root = data_root()?;
    fs::create_dir_all(&root)
        .map_err(|error| format!("Failed to create {}: {error}", root.display()))?;
    ensure_default_categories(&root)?;
    let categories = category_directories(&root)?;
    let selected_categories = if let Some(query) = category {
        vec![resolve_category_query(&categories, query)?.clone()]
    } else {
        categories
    };
    let mut ideas = Vec::new();
    for directory in selected_categories {
        let folder = root.join(&directory);
        for entry in fs::read_dir(&folder)
            .map_err(|error| format!("Failed to list {}: {error}", folder.display()))?
        {
            let entry =
                entry.map_err(|error| format!("Failed to read {}: {error}", folder.display()))?;
            let path = entry.path();
            if !path.is_file()
                || path.extension().and_then(|extension| extension.to_str()) != Some("md")
            {
                continue;
            }
            let relative = path
                .strip_prefix(&root)
                .map_err(|error| format!("Invalid idea file path: {error}"))?
                .to_string_lossy()
                .replace('\\', "/");
            let parsed = parse_file(relative, &path)?;
            ideas.push(CliIdea {
                filepath: parsed.filepath,
                metadata: parsed.metadata,
            });
        }
    }
    ideas.sort_by(|left, right| {
        left.metadata
            .title
            .to_lowercase()
            .cmp(&right.metadata.title.to_lowercase())
    });
    Ok(ideas)
}

pub fn cli_delete_idea(filepath: &str) -> Result<(), String> {
    let root = data_root()?;
    let path = idea_path(&root, filepath)?;
    fs::remove_file(&path).map_err(|error| format!("Failed to delete {}: {error}", path.display()))
}
