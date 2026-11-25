use std::path::PathBuf;

use tauri_plugin_opener::OpenerExt;

use crate::{icon::read_icon_data_url, url::validate_http_url};

#[tauri::command]
pub fn open_resource(app: tauri::AppHandle, path: String) -> Result<(), String> {
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
pub fn read_icon(path: String) -> Result<String, String> {
    read_icon_data_url(path)
}

#[tauri::command]
pub fn open_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let normalized_url = validate_http_url(&url)?;

    app.opener()
        .open_url(normalized_url, None::<&str>)
        .map_err(|error| format!("Failed to open URL: {error}"))
}