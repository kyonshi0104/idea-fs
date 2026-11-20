use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
use tauri::{
    Manager, WebviewUrl, WebviewWindowBuilder,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_opener::OpenerExt;
use url::Url;

const DEFAULT_GLOBAL_SHORTCUT: &str = "CommandOrControl+Alt+I";
const DEFAULT_CATEGORIES: [(&str, &str); 4] = [
    ("01_Inbox", "inbox"),
    ("02_Backlog", "backlog"),
    ("03_WIP", "wip"),
    ("04_Graveyard", "graveyard"),
];
const MAX_ICON_BYTES: u64 = 5 * 1024 * 1024;

fn category_status(directory: &str) -> String {
    match category_prefix(directory) {
        Some(1) => "inbox".to_owned(),
        Some(2) => "backlog".to_owned(),
        Some(3) => "wip".to_owned(),
        Some(4) => "graveyard".to_owned(),
        _ => directory.to_owned(),
    }
}

fn is_default_category(directory: &str) -> bool {
    let status = category_status(directory);
    DEFAULT_CATEGORIES
        .iter()
        .any(|(_, default_status)| *default_status == status)
}

fn category_prefix(directory: &str) -> Option<u64> {
    let (prefix, _) = directory.split_once('_')?;
    prefix.parse().ok()
}

fn category_display_name(directory: &str) -> &str {
    directory
        .split_once('_')
        .map(|(_, name)| name)
        .unwrap_or(directory)
}

fn sort_categories(categories: &mut [String]) {
    categories.sort_by(|left, right| {
        match (category_prefix(left), category_prefix(right)) {
            (Some(left), Some(right)) => left.cmp(&right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
        .then_with(|| left.to_lowercase().cmp(&right.to_lowercase()))
    });
}

fn ensure_default_categories(root: &Path) -> Result<(), String> {
    let existing = category_directories(root)?;
    for (directory, status) in DEFAULT_CATEGORIES {
        if existing
            .iter()
            .any(|category| category_status(category) == status)
        {
            continue;
        }
        let path = root.join(directory);
        fs::create_dir_all(&path)
            .map_err(|error| format!("Failed to create {}: {error}", path.display()))?;
    }
    Ok(())
}

fn category_directories(root: &Path) -> Result<Vec<String>, String> {
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("Failed to access idea data directory: {error}"))?;
    let entries = fs::read_dir(root)
        .map_err(|error| format!("Failed to list {}: {error}", root.display()))?;
    let mut categories = Vec::new();
    for entry in entries {
        let entry = entry
            .map_err(|error| format!("Failed to read an entry in {}: {error}", root.display()))?;
        if !entry
            .file_type()
            .map_err(|error| format!("Failed to inspect {}: {error}", entry.path().display()))?
            .is_dir()
        {
            continue;
        }

        let name = entry.file_name().into_string().map_err(|_| {
            format!(
                "Category name is not valid Unicode: {}",
                entry.path().display()
            )
        })?;
        let canonical_category = entry
            .path()
            .canonicalize()
            .map_err(|error| format!("Failed to access {}: {error}", entry.path().display()))?;
        if !canonical_category.starts_with(&canonical_root) {
            return Err(format!(
                "Category directory is outside the data directory: {}",
                entry.path().display()
            ));
        }
        categories.push(name);
    }
    sort_categories(&mut categories);
    Ok(categories)
}

fn sanitize_category_name(name: &str) -> Result<String, String> {
    let sanitized = name
        .trim()
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
        return Err("Category name cannot be empty".to_owned());
    }
    Ok(sanitized.to_owned())
}

fn create_category_directory(root: &Path, name: &str) -> Result<String, String> {
    let name = sanitize_category_name(name)?;
    let categories = category_directories(root)?;
    let next_prefix = categories
        .iter()
        .filter_map(|directory| category_prefix(directory))
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| "No category prefix numbers are available".to_owned())?;
    let directory = format!("{next_prefix:02}_{name}");
    let path = root.join(&directory);
    fs::create_dir(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            format!("Category already exists: {}", path.display())
        } else {
            format!("Failed to create {}: {error}", path.display())
        }
    })?;
    Ok(directory)
}

fn rewrite_frontmatter_status(raw: &str, status: &str) -> Result<String, String> {
    let frontmatter = raw
        .strip_prefix("---\n")
        .or_else(|| raw.strip_prefix("---\r\n"))
        .ok_or_else(|| "Idea has no YAML frontmatter".to_owned())?;
    let (yaml, body_suffix) = frontmatter
        .split_once("\n---")
        .ok_or_else(|| "Idea has an unterminated YAML frontmatter".to_owned())?;
    let mut metadata: MetaData =
        serde_yaml::from_str(yaml).map_err(|error| format!("Invalid YAML frontmatter: {error}"))?;
    metadata.status = status.to_owned();
    let yaml = serde_yaml::to_string(&metadata)
        .map_err(|error| format!("Failed to serialize idea metadata: {error}"))?;
    Ok(format!("---\n{yaml}---{body_suffix}"))
}

fn rename_category_directory(
    root: &Path,
    current_directory: &str,
    name: &str,
) -> Result<String, String> {
    let categories = category_directories(root)?;
    if !categories
        .iter()
        .any(|category| category == current_directory)
    {
        return Err("Category does not exist".to_owned());
    }

    let name = sanitize_category_name(name)?;
    let prefix = current_directory
        .split_once('_')
        .map(|(prefix, _)| format!("{prefix}_"))
        .unwrap_or_default();
    let new_directory = format!("{prefix}{name}");
    if new_directory == current_directory {
        return Ok(new_directory);
    }

    let source = root.join(current_directory);
    let target = root.join(&new_directory);
    if target.exists() {
        return Err(format!("Category already exists: {}", target.display()));
    }

    let mut updates = Vec::new();
    if !is_default_category(current_directory) {
        let entries = fs::read_dir(&source)
            .map_err(|error| format!("Failed to list {}: {error}", source.display()))?;
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("Failed to read {}: {error}", source.display()))?;
            let path = entry.path();
            if !path.is_file()
                || path.extension().and_then(|extension| extension.to_str()) != Some("md")
            {
                continue;
            }

            let original = fs::read_to_string(&path)
                .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
            let updated = rewrite_frontmatter_status(&original, &new_directory)?;
            let filename = path
                .file_name()
                .ok_or_else(|| "Invalid category filename".to_owned())?
                .to_owned();
            updates.push((filename, original, updated));
        }
    }

    fs::rename(&source, &target).map_err(|error| {
        format!(
            "Failed to rename category {} to {}: {error}",
            source.display(),
            target.display()
        )
    })?;

    for (filename, _, updated) in &updates {
        if let Err(error) = fs::write(target.join(filename), updated) {
            let mut rollback_errors = Vec::new();
            for (original_filename, original, _) in &updates {
                if let Err(rollback_error) = fs::write(target.join(original_filename), original) {
                    rollback_errors.push(format!(
                        "Failed to restore {}: {rollback_error}",
                        target.join(original_filename).display()
                    ));
                }
            }
            if let Err(rollback_error) = fs::rename(&target, &source) {
                rollback_errors.push(format!("Failed to restore category name: {rollback_error}"));
            }
            let rollback = if rollback_errors.is_empty() {
                String::new()
            } else {
                format!("; rollback errors: {}", rollback_errors.join("; "))
            };
            return Err(format!(
                "Failed to update renamed category metadata: {error}{rollback}"
            ));
        }
    }

    Ok(new_directory)
}

fn delete_category_directory(root: &Path, directory: &str) -> Result<(), String> {
    if !category_directories(root)?
        .iter()
        .any(|category| category == directory)
    {
        return Err("Category does not exist".to_owned());
    }
    if is_default_category(directory) {
        return Err("Built-in categories cannot be deleted".to_owned());
    }

    let path = root.join(directory);
    let mut entries = fs::read_dir(&path)
        .map_err(|error| format!("Failed to list {}: {error}", path.display()))?;
    match entries.next() {
        Some(Ok(_)) => return Err("Only empty categories can be deleted".to_owned()),
        Some(Err(error)) => {
            return Err(format!("Failed to read {}: {error}", path.display()));
        }
        None => {}
    }
    fs::remove_dir(&path).map_err(|error| format!("Failed to delete {}: {error}", path.display()))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MetaData {
    pub title: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub status: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Serialize)]
pub struct IdeaFile {
    filepath: String,
    metadata: MetaData,
}

#[derive(Serialize)]
pub struct IdeaFolder {
    name: String,
    status: String,
    files: Vec<IdeaFile>,
}

#[derive(Serialize)]
pub struct TreeData {
    folders: Vec<IdeaFolder>,
}

#[derive(Serialize)]
pub struct FileData {
    filepath: String,
    metadata: MetaData,
    content: String,
}

#[tauri::command]
fn create_idea(
    app: tauri::AppHandle,
    title: String,
    tags: Vec<String>,
    status: String,
    content: String,
    icon: Option<String>,
    resource_path: Option<String>,
    app_type: Option<String>,
    url: Option<String>,
) -> Result<String, String> {
    let root = app_data_root(&app)?;
    fs::create_dir_all(&root)
        .map_err(|error| format!("Failed to create {}: {error}", root.display()))?;
    ensure_default_categories(&root)?;
    let categories = category_directories(&root)?;
    let directory = categories
        .iter()
        .find(|directory| category_status(directory) == status)
        .cloned()
        .ok_or_else(|| "Invalid idea status".to_owned())?;
    create_idea_in_directory(
        &root,
        &directory,
        &title,
        tags,
        content,
        icon,
        resource_path,
        app_type,
        url,
    )
}

fn create_idea_in_directory(
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

    let now = Local::now();
    let metadata = MetaData {
        title: title.to_owned(),
        tags: tags
            .into_iter()
            .map(|tag| tag.trim().to_owned())
            .filter(|tag| !tag.is_empty())
            .collect(),
        status,
        created_at: now.to_rfc3339(),
        icon: non_empty(icon),
        resource_path: non_empty(resource_path),
        app_type: non_empty(app_type),
        url: non_empty(url),
    };
    let yaml = serde_yaml::to_string(&metadata)
        .map_err(|error| format!("Failed to serialize idea metadata: {error}"))?;
    let markdown = format!("---\n{yaml}---\n\n{content}");
    let stem = format!(
        "{}_{}",
        now.format("%Y%m%d%H%M%S"),
        sanitize_filename(title)
    );

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

#[derive(Clone, Debug)]
pub struct CliIdea {
    pub filepath: String,
    pub metadata: MetaData,
}

fn resolve_category_query<'a>(categories: &'a [String], query: &str) -> Result<&'a String, String> {
    let matches = categories
        .iter()
        .filter(|category| {
            category_display_name(category)
                .to_lowercase()
                .contains(&query.trim().to_lowercase())
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [category] => Ok(category),
        [] => Err(format!("No category matches \"{query}\"")),
        _ => Err(format!(
            "Category query \"{query}\" is ambiguous: {}",
            matches
                .iter()
                .map(|category| category_display_name(category).to_owned())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

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

#[tauri::command]
fn create_category(app: tauri::AppHandle, name: String) -> Result<String, String> {
    let root = app_data_root(&app)?;
    fs::create_dir_all(&root)
        .map_err(|error| format!("Failed to create {}: {error}", root.display()))?;
    ensure_default_categories(&root)?;
    create_category_directory(&root, &name)
}

#[tauri::command]
fn rename_category(
    app: tauri::AppHandle,
    current_dir: String,
    name: String,
) -> Result<String, String> {
    let root = app_data_root(&app)?;
    fs::create_dir_all(&root)
        .map_err(|error| format!("Failed to create {}: {error}", root.display()))?;
    ensure_default_categories(&root)?;
    rename_category_directory(&root, &current_dir, &name)
}

#[tauri::command]
fn delete_category(app: tauri::AppHandle, category_dir: String) -> Result<(), String> {
    let root = app_data_root(&app)?;
    fs::create_dir_all(&root)
        .map_err(|error| format!("Failed to create {}: {error}", root.display()))?;
    ensure_default_categories(&root)?;
    delete_category_directory(&root, &category_dir)
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[tauri::command]
fn open_resource(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let resource_path = PathBuf::from(&path);
    if !resource_path.is_absolute() {
        return Err("Resource path must be absolute".to_owned());
    }
    if !resource_path.exists() {
        return Err(format!("Resource does not exist: {path}"));
    }

    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|error| format!("Failed to open resource: {error}"))
}

#[tauri::command]
fn read_icon(path: String) -> Result<String, String> {
    let path = PathBuf::from(path);
    if !path.is_absolute() || !path.is_file() {
        return Err("Icon path must be an existing absolute file path".to_owned());
    }
    let mime_type = image_mime_type(&path)
        .ok_or_else(|| "Icon must be a PNG, JPEG, GIF, WebP, AVIF, or SVG image".to_owned())?;
    let file = fs::File::open(&path)
        .map_err(|error| format!("Failed to read icon {}: {error}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(MAX_ICON_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Failed to read icon {}: {error}", path.display()))?;
    if bytes.len() as u64 > MAX_ICON_BYTES {
        return Err("Icon image exceeds the 5 MiB limit".to_owned());
    }
    Ok(format!("data:{mime_type};base64,{}", BASE64.encode(bytes)))
}

fn image_mime_type(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "avif" => Some("image/avif"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

#[tauri::command]
fn open_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let normalized_url = validate_http_url(&url)?;

    app.opener()
        .open_url(normalized_url, None::<&str>)
        .map_err(|error| format!("Failed to open URL: {error}"))
}

fn validate_http_url(url: &str) -> Result<String, String> {
    let parsed = Url::parse(url).map_err(|error| format!("Invalid URL: {error}"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("Only HTTP and HTTPS URLs can be opened".to_owned());
    }
    Ok(parsed.to_string())
}

pub fn data_root() -> Result<PathBuf, String> {
    dirs::document_dir()
        .map(|directory| directory.join("IdeaFS"))
        .ok_or_else(|| "Could not resolve the user's Documents directory".to_owned())
}

fn app_data_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .document_dir()
        .map(|directory| directory.join("IdeaFS"))
        .map_err(|error| format!("Failed to resolve the user's Documents directory: {error}"))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
struct AppConfig {
    #[serde(default = "default_global_shortcut")]
    global_shortcut: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            global_shortcut: default_global_shortcut(),
        }
    }
}

fn default_global_shortcut() -> Option<String> {
    Some(DEFAULT_GLOBAL_SHORTCUT.to_owned())
}

fn app_config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("Failed to resolve the app config directory: {error}"))?;
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Failed to create {}: {error}", directory.display()))?;
    Ok(directory.join("settings.json"))
}

fn read_app_config(app: &tauri::AppHandle) -> Result<AppConfig, String> {
    let path = app_config_path(app)?;
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content)
            .map_err(|error| format!("Invalid settings in {}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(AppConfig::default()),
        Err(error) => Err(format!("Failed to read {}: {error}", path.display())),
    }
}

fn write_app_config(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let path = app_config_path(app)?;
    let content = serde_json::to_string_pretty(config)
        .map_err(|error| format!("Failed to serialize settings: {error}"))?;
    fs::write(&path, content)
        .map_err(|error| format!("Failed to write {}: {error}", path.display()))
}

fn show_main_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .show()
            .map_err(|error| format!("Failed to show the main window: {error}"))?;
        window
            .set_focus()
            .map_err(|error| format!("Failed to focus the main window: {error}"))?;
        return Ok(());
    }

    WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("IdeaFS")
        .inner_size(1290.0, 750.0)
        .build()
        .map_err(|error| format!("Failed to create the main window: {error}"))?;
    Ok(())
}

fn show_quick_add_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("quick-add") {
        window
            .show()
            .map_err(|error| format!("Failed to show Quick Add: {error}"))?;
        window
            .set_focus()
            .map_err(|error| format!("Failed to focus Quick Add: {error}"))?;
        return Ok(());
    }

    WebviewWindowBuilder::new(app, "quick-add", WebviewUrl::App("index.html".into()))
        .title("IdeaFS Quick Add")
        .inner_size(450.0, 60.0)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .center()
        .build()
        .map_err(|error| format!("Failed to create Quick Add: {error}"))?;
    Ok(())
}

fn create_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let open = MenuItem::with_id(app, "open", "Open IdeaFS", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("Default application icon is not configured")?;

    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                if let Err(error) = show_main_window(app) {
                    eprintln!("{error}");
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

fn register_saved_shortcut(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(shortcut) = read_app_config(app)?.global_shortcut {
        app.global_shortcut()
            .register(shortcut.as_str())
            .map_err(|error| format!("Failed to register global shortcut `{shortcut}`: {error}"))?;
    }
    Ok(())
}

fn restore_shortcut(app: &tauri::AppHandle, shortcut: Option<&str>) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|error| format!("Failed to unregister global shortcuts: {error}"))?;
    if let Some(shortcut) = shortcut {
        app.global_shortcut()
            .register(shortcut)
            .map_err(|error| format!("Failed to restore global shortcut `{shortcut}`: {error}"))?;
    }
    Ok(())
}

#[tauri::command]
fn get_global_shortcut(app: tauri::AppHandle) -> Result<Option<String>, String> {
    Ok(read_app_config(&app)?.global_shortcut)
}

#[tauri::command]
fn set_global_shortcut(app: tauri::AppHandle, shortcut: Option<String>) -> Result<(), String> {
    let config = read_app_config(&app)?;
    let previous = config.global_shortcut;
    restore_shortcut(&app, shortcut.as_deref()).map_err(|error| {
        let rollback = restore_shortcut(&app, previous.as_deref())
            .err()
            .map(|error| format!("; failed to restore previous shortcut: {error}"))
            .unwrap_or_default();
        format!("{error}{rollback}")
    })?;

    let updated = AppConfig {
        global_shortcut: shortcut,
    };
    if let Err(error) = write_app_config(&app, &updated) {
        let rollback = restore_shortcut(&app, previous.as_deref())
            .err()
            .map(|error| format!("; failed to restore previous shortcut: {error}"))
            .unwrap_or_default();
        return Err(format!("{error}{rollback}"));
    }
    Ok(())
}

#[derive(Serialize)]
struct AppSettings {
    data_directory: String,
    supports_mac_cli_install: bool,
    mac_cli_installed: bool,
}

#[tauri::command]
fn get_app_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    let data_directory = app_data_root(&app)?;
    #[cfg(target_os = "macos")]
    let mac_cli_installed = {
        let link = Path::new("/usr/local/bin/idfs");
        match fs::read_link(link) {
            Ok(target) => {
                let executable = mac_cli_executable()?;
                target == executable
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(format!("Failed to inspect {}: {error}", link.display())),
        }
    };
    #[cfg(not(target_os = "macos"))]
    let mac_cli_installed = false;

    Ok(AppSettings {
        data_directory: data_directory.to_string_lossy().into_owned(),
        supports_mac_cli_install: cfg!(target_os = "macos"),
        mac_cli_installed,
    })
}

#[tauri::command]
fn install_mac_cli_command() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::symlink;

        let executable = mac_cli_executable()?;
        let link = Path::new("/usr/local/bin/idfs");
        if let Ok(existing_target) = fs::read_link(link) {
            if existing_target == executable {
                return Ok(());
            }
        }
        match fs::symlink_metadata(link) {
            Ok(_) => {
                return Err(format!(
                    "{} already exists and does not point to this application",
                    link.display()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!("Failed to inspect {}: {error}", link.display()));
            }
        }
        fs::create_dir_all("/usr/local/bin")
            .map_err(|error| format!("Failed to create /usr/local/bin: {error}"))?;
        symlink(&executable, link).map_err(|error| {
            format!(
                "Failed to install {}: {error}. You may need permission to write /usr/local/bin.",
                link.display()
            )
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("The CLI PATH installer is only available on macOS".to_owned())
    }
}

#[cfg(target_os = "macos")]
fn mac_cli_executable() -> Result<PathBuf, String> {
    let gui_executable = std::env::current_exe()
        .map_err(|error| format!("Failed to locate the application executable: {error}"))?;
    let directory = gui_executable
        .parent()
        .ok_or_else(|| "Application executable has no parent directory".to_owned())?;
    let cli_executable = directory.join("idfs");
    if !cli_executable.is_file() {
        return Err(format!(
            "IdeaFS CLI executable was not found: {}",
            cli_executable.display()
        ));
    }
    Ok(cli_executable)
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

fn idea_path(root: &Path, filepath: &str) -> Result<PathBuf, String> {
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
        .ok_or_else(|| "Idea paths must point to a Markdown file in a status folder".to_owned())?;
    if components.next().is_some()
        || Path::new(filename.as_os_str())
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

fn parse_file(filepath: String, path: &Path) -> Result<FileData, String> {
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

#[tauri::command]
fn get_idea_tree(app: tauri::AppHandle) -> Result<TreeData, String> {
    let root = app_data_root(&app)?;
    fs::create_dir_all(&root)
        .map_err(|error| format!("Failed to create {}: {error}", root.display()))?;
    ensure_default_categories(&root)?;
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("Failed to access idea data directory: {error}"))?;
    let categories = category_directories(&root)?;
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
                    .strip_prefix(&root)
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

#[tauri::command]
fn read_idea_file(app: tauri::AppHandle, filepath: String) -> Result<FileData, String> {
    let root = app_data_root(&app)?;
    let path = idea_path(&root, &filepath)?;
    parse_file(filepath, &path)
}

#[tauri::command]
fn delete_idea_file(app: tauri::AppHandle, filepath: String) -> Result<(), String> {
    let root = app_data_root(&app)?;
    let path = idea_path(&root, &filepath)?;
    fs::remove_file(&path).map_err(|error| format!("Failed to delete {}: {error}", path.display()))
}

#[tauri::command]
fn save_idea_file(
    app: tauri::AppHandle,
    filepath: String,
    metadata: MetaData,
    content: String,
) -> Result<(), String> {
    let root = app_data_root(&app)?;
    let path = idea_path(&root, &filepath)?;
    let status_dir = Path::new(&filepath)
        .components()
        .next()
        .and_then(|component| component.as_os_str().to_str())
        .ok_or_else(|| "Invalid idea file path".to_owned())?;
    let expected_status = category_status(status_dir);
    if metadata.status != expected_status {
        return Err("Move the idea to its selected status before saving".to_owned());
    }
    let yaml = serde_yaml::to_string(&metadata)
        .map_err(|error| format!("Failed to serialize idea metadata: {error}"))?;
    let file_content = format!("---\n{yaml}---\n\n{content}");
    fs::write(&path, file_content)
        .map_err(|error| format!("Failed to save {}: {error}", path.display()))
}

#[tauri::command]
fn move_idea_file(
    app: tauri::AppHandle,
    source_path: String,
    target_dir: String,
) -> Result<String, String> {
    let root = app_data_root(&app)?;
    let source = idea_path(&root, &source_path)?;
    if !category_directories(&root)?
        .iter()
        .any(|name| name == &target_dir)
    {
        return Err("Invalid target category folder".to_owned());
    }
    let target_status = category_status(&target_dir);
    let mut file = parse_file(source_path, &source)?;
    let filename = source
        .file_name()
        .ok_or_else(|| "Invalid idea filename".to_owned())?;
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("Failed to access idea data directory: {error}"))?;
    let target_folder = root.join(&target_dir);
    let canonical_target_folder = target_folder
        .canonicalize()
        .map_err(|error| format!("Failed to access {}: {error}", target_folder.display()))?;
    if !canonical_target_folder.starts_with(&canonical_root) {
        return Err("Target status folder is outside the data directory".to_owned());
    }
    let target = canonical_target_folder.join(filename);
    if target.exists() {
        return Err(format!(
            "The destination already exists: {}",
            target.display()
        ));
    }
    file.metadata.status = target_status;
    let yaml = serde_yaml::to_string(&file.metadata)
        .map_err(|error| format!("Failed to serialize idea metadata: {error}"))?;
    let content = format!("---\n{yaml}---\n\n{}", file.content);
    fs::rename(&source, &target).map_err(|error| {
        format!(
            "Failed to move {} to {}: {error}",
            source.display(),
            target.display()
        )
    })?;
    if let Err(error) = fs::write(&target, content) {
        return match fs::rename(&target, &source) {
            Ok(()) => Err(format!("Failed to update moved idea metadata: {error}")),
            Err(rollback_error) => Err(format!(
                "Failed to update moved idea metadata: {error}; rollback to {} also failed: {rollback_error}",
                source.display()
            )),
        };
    }
    Ok(target
        .strip_prefix(&canonical_root)
        .map_err(|error| format!("Invalid destination path: {error}"))?
        .to_string_lossy()
        .replace('\\', "/"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Err(error) = show_main_window(app) {
                eprintln!("{error}");
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed
                        && let Err(error) = show_quick_add_window(app)
                    {
                        eprintln!("{error}");
                    }
                })
                .build(),
        )
        .setup(|app| {
            if let Err(error) = register_saved_shortcut(app.handle()) {
                eprintln!("{error}");
            }
            create_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            create_idea,
            create_category,
            rename_category,
            delete_category,
            get_global_shortcut,
            set_global_shortcut,
            get_idea_tree,
            read_idea_file,
            save_idea_file,
            move_idea_file,
            delete_idea_file,
            read_icon,
            open_resource,
            open_url,
            get_app_settings,
            install_mac_cli_command
        ])
        .build(tauri::generate_context!())
        .expect("error while building Tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::{
        MetaData, category_prefix, category_status, create_category_directory, data_root,
        delete_category_directory, ensure_default_categories, image_mime_type,
        rename_category_directory, sanitize_category_name, sort_categories, validate_http_url,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            static NEXT_ID: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "ideafs-category-test-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("temporary test directory should be created");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn cli_data_root_uses_documents_idea_explorer_directory() {
        assert_eq!(
            data_root().expect("Documents directory should be available"),
            dirs::document_dir()
                .expect("Documents directory should be available")
                .join("IdeaFS")
        );
    }

    #[test]
    fn legacy_frontmatter_defaults_new_fields_to_none() {
        let metadata: MetaData = serde_yaml::from_str(
            "title: Old idea\ntags: []\nstatus: inbox\ncreated_at: 2026-01-01T00:00:00Z\n",
        )
        .expect("legacy metadata should remain readable");

        assert_eq!(metadata.icon, None);
        assert_eq!(metadata.resource_path, None);
        assert_eq!(metadata.app_type, None);
        assert_eq!(metadata.url, None);
    }

    #[test]
    fn extended_frontmatter_reads_generic_resource_fields() {
        let metadata: MetaData = serde_yaml::from_str(
            "title: Music idea\ntags: [music]\nstatus: wip\ncreated_at: 2026-01-01T00:00:00Z\nicon: 🎹\nresource_path: C:/Music/Track.als\napp_type: ableton\nurl: https://example.com\n",
        )
        .expect("extended metadata should be readable");

        assert_eq!(metadata.icon.as_deref(), Some("🎹"));
        assert_eq!(
            metadata.resource_path.as_deref(),
            Some("C:/Music/Track.als")
        );
        assert_eq!(metadata.app_type.as_deref(), Some("ableton"));
        assert_eq!(metadata.url.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn url_opener_accepts_http_and_rejects_other_schemes() {
        assert_eq!(
            validate_http_url("https://example.com/path").as_deref(),
            Ok("https://example.com/path")
        );
        assert!(validate_http_url("file:///C:/secret.txt").is_err());
        assert!(validate_http_url("javascript:alert(1)").is_err());
    }

    #[test]
    fn icon_mime_type_is_limited_to_supported_image_formats() {
        assert_eq!(image_mime_type(Path::new("cover.PNG")), Some("image/png"));
        assert_eq!(image_mime_type(Path::new("cover.jpeg")), Some("image/jpeg"));
        assert_eq!(
            image_mime_type(Path::new("cover.svg")),
            Some("image/svg+xml")
        );
        assert_eq!(image_mime_type(Path::new("notes.md")), None);
    }

    #[test]
    fn category_prefixes_sort_before_unprefixed_categories() {
        let mut categories = vec![
            "Notes".to_owned(),
            "10_Archive".to_owned(),
            "02_Backlog".to_owned(),
            "01_Inbox".to_owned(),
        ];

        sort_categories(&mut categories);

        assert_eq!(
            categories,
            ["01_Inbox", "02_Backlog", "10_Archive", "Notes"]
        );
        assert_eq!(category_prefix("10_Archive"), Some(10));
        assert_eq!(category_prefix("Notes"), None);
    }

    #[test]
    fn category_names_are_sanitized_and_nonempty() {
        assert_eq!(
            sanitize_category_name("  Art/Ideas. ").as_deref(),
            Ok("Art_Ideas")
        );
        assert!(sanitize_category_name(" .. ").is_err());
    }

    #[test]
    fn new_category_uses_next_number_after_highest_prefix() {
        let root = TestDirectory::new();
        fs::create_dir(root.0.join("01_Inbox")).expect("default category should be created");
        fs::create_dir(root.0.join("07_Archive")).expect("numbered category should be created");
        fs::create_dir(root.0.join("Notes")).expect("unprefixed category should be created");

        let name =
            create_category_directory(&root.0, "Art/Ideas").expect("category should be created");

        assert_eq!(name, "08_Art_Ideas");
        assert!(root.0.join(name).is_dir());
    }

    #[test]
    fn category_rename_updates_custom_statuses_and_preserves_files() {
        let root = TestDirectory::new();
        let old_directory = root.0.join("05_Research");
        fs::create_dir(&old_directory).expect("category should be created");
        fs::write(
            old_directory.join("idea.md"),
            "---\ntitle: Research\ntags: []\nstatus: 05_Research\ncreated_at: 2026-01-01T00:00:00Z\n---\n\nNotes\n",
        )
        .expect("idea should be created");

        let new_name = rename_category_directory(&root.0, "05_Research", "Reading")
            .expect("category should be renamed");
        let renamed_file = root.0.join(&new_name).join("idea.md");
        let renamed = super::parse_file(format!("{new_name}/idea.md"), &renamed_file)
            .expect("renamed idea should be readable");

        assert_eq!(new_name, "05_Reading");
        assert_eq!(renamed.metadata.status, "05_Reading");
        assert_eq!(renamed.content, "\nNotes\n");
        assert!(!old_directory.exists());
    }

    #[test]
    fn renaming_a_default_category_keeps_its_status_and_prevents_reseeding() {
        let root = TestDirectory::new();
        fs::create_dir(root.0.join("01_Inbox")).expect("default category should be created");

        let renamed = rename_category_directory(&root.0, "01_Inbox", "Capture")
            .expect("default category should be renamed");
        ensure_default_categories(&root.0).expect("missing defaults should be seeded");

        assert_eq!(renamed, "01_Capture");
        assert_eq!(category_status(&renamed), "inbox");
        assert!(!root.0.join("01_Inbox").exists());
    }

    #[test]
    fn category_delete_rejects_builtins_and_nonempty_categories() {
        let root = TestDirectory::new();
        fs::create_dir(root.0.join("01_Inbox")).expect("default category should be created");
        let custom = root.0.join("05_Notes");
        fs::create_dir(&custom).expect("custom category should be created");
        fs::write(custom.join("idea.md"), "idea").expect("file should be created");

        assert!(delete_category_directory(&root.0, "01_Inbox").is_err());
        assert!(
            delete_category_directory(&root.0, "05_Notes")
                .expect_err("nonempty category should not be deleted")
                .contains("Only empty categories")
        );

        fs::remove_file(custom.join("idea.md")).expect("file should be removed");
        delete_category_directory(&root.0, "05_Notes").expect("empty category should be deleted");
        assert!(!custom.exists());
    }
}
