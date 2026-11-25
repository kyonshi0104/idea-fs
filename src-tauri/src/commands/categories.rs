use ideafs_core::{
    create_category_directory, delete_category_directory, rename_category_directory,
};

use crate::paths::prepare_data_root;

#[tauri::command]
pub fn create_category(app: tauri::AppHandle, name: String) -> Result<String, String> {
    let root = prepare_data_root(&app)?;
    create_category_directory(&root, &name)
}

#[tauri::command]
pub fn rename_category(
    app: tauri::AppHandle,
    current_dir: String,
    name: String,
) -> Result<String, String> {
    let root = prepare_data_root(&app)?;
    rename_category_directory(&root, &current_dir, &name)
}

#[tauri::command]
pub fn delete_category(app: tauri::AppHandle, category_dir: String) -> Result<(), String> {
    let root = prepare_data_root(&app)?;
    delete_category_directory(&root, &category_dir)
}