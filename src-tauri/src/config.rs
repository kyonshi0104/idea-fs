use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::Manager;

pub(crate) const DEFAULT_GLOBAL_SHORTCUT: &str = "CommandOrControl+Alt+I";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct AppConfig {
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

impl AppConfig {
    pub(crate) fn global_shortcut(&self) -> &Option<String> {
        &self.global_shortcut
    }

    pub(crate) fn with_global_shortcut(global_shortcut: Option<String>) -> Self {
        Self { global_shortcut }
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

pub(crate) fn read_app_config(app: &tauri::AppHandle) -> Result<AppConfig, String> {
    let path = app_config_path(app)?;
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content)
            .map_err(|error| format!("Invalid settings in {}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(AppConfig::default()),
        Err(error) => Err(format!("Failed to read {}: {error}", path.display())),
    }
}

pub(crate) fn write_app_config(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let path = app_config_path(app)?;
    let content = serde_json::to_string_pretty(config)
        .map_err(|error| format!("Failed to serialize settings: {error}"))?;
    fs::write(&path, content)
        .map_err(|error| format!("Failed to write {}: {error}", path.display()))
}
