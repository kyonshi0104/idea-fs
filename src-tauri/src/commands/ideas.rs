use std::{fs, path::Path};

use ideafs_core::{
    category_directories, category_status, collect_idea_tree, create_idea_in_directory, idea_path,
    parse_file, resolve_status_directory, FileData, MetaData, TreeData,
};

use crate::paths::prepare_data_root;

#[tauri::command]
pub fn create_idea(
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
    let root = prepare_data_root(&app)?;
    let directory = resolve_status_directory(&root, &status)?;
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

#[tauri::command]
pub fn get_idea_tree(app: tauri::AppHandle) -> Result<TreeData, String> {
    let root = prepare_data_root(&app)?;
    collect_idea_tree(&root)
}

#[tauri::command]
pub fn read_idea_file(app: tauri::AppHandle, filepath: String) -> Result<FileData, String> {
    let root = prepare_data_root(&app)?;
    let path = idea_path(&root, &filepath)?;
    parse_file(filepath, &path)
}

#[tauri::command]
pub fn delete_idea_file(app: tauri::AppHandle, filepath: String) -> Result<(), String> {
    let root = prepare_data_root(&app)?;
    let path = idea_path(&root, &filepath)?;
    fs::remove_file(&path).map_err(|error| format!("Failed to delete {}: {error}", path.display()))
}

#[tauri::command]
pub fn save_idea_file(
    app: tauri::AppHandle,
    filepath: String,
    metadata: MetaData,
    content: String,
) -> Result<(), String> {
    let root = prepare_data_root(&app)?;
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
pub fn move_idea_file(
    app: tauri::AppHandle,
    source_path: String,
    target_dir: String,
) -> Result<String, String> {
    let root = prepare_data_root(&app)?;
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