use std::{fs, path::PathBuf};

use ideafs_core::ensure_default_categories;

pub(crate) fn prepare_data_root(_app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let root = ideafs_core::data_root()?;
    fs::create_dir_all(&root)
        .map_err(|error| format!("Failed to create {}: {error}", root.display()))?;
    ensure_default_categories(&root)?;
    Ok(root)
}
