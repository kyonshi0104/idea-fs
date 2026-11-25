use std::path::PathBuf;

use tauri::Manager;

pub fn data_root() -> Result<PathBuf, String> {
    dirs::document_dir()
        .map(|directory| directory.join("IdeaFS"))
        .ok_or_else(|| "Could not resolve the user's Documents directory".to_owned())
}

pub(crate) fn app_data_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .document_dir()
        .map(|directory| directory.join("IdeaFS"))
        .map_err(|error| format!("Failed to resolve the user's Documents directory: {error}"))
}