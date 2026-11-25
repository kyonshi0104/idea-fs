use serde::Serialize;

use crate::{
    config::{AppConfig, read_app_config, write_app_config},
    shortcuts::restore_shortcut,
    mac_cli::{install_mac_cli, is_mac_cli_installed},
    paths::app_data_root,
};

#[derive(Serialize)]
pub struct AppSettings {
    data_directory: String,
    supports_mac_cli_install: bool,
    mac_cli_installed: bool,
}

#[tauri::command]
pub fn get_global_shortcut(app: tauri::AppHandle) -> Result<Option<String>, String> {
    Ok(read_app_config(&app)?.global_shortcut().clone())
}

#[tauri::command]
pub fn set_global_shortcut(app: tauri::AppHandle, shortcut: Option<String>) -> Result<(), String> {
    let config = read_app_config(&app)?;
    let previous = config.global_shortcut().clone();
    restore_shortcut(&app, shortcut.as_deref()).map_err(|error| {
        let rollback = restore_shortcut(&app, previous.as_deref())
            .err()
            .map(|error| format!("; failed to restore previous shortcut: {error}"))
            .unwrap_or_default();
        format!("{error}{rollback}")
    })?;

    let updated = AppConfig::with_global_shortcut(shortcut);
    if let Err(error) = write_app_config(&app, &updated) {
        let rollback = restore_shortcut(&app, previous.as_deref())
            .err()
            .map(|error| format!("; failed to restore previous shortcut: {error}"))
            .unwrap_or_default();
        return Err(format!("{error}{rollback}"));
    }
    Ok(())
}

#[tauri::command]
pub fn get_app_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    let data_directory = app_data_root(&app)?;

    Ok(AppSettings {
        data_directory: data_directory.to_string_lossy().into_owned(),
        supports_mac_cli_install: cfg!(target_os = "macos"),
        mac_cli_installed: is_mac_cli_installed()?,
    })
}

#[tauri::command]
pub fn install_mac_cli_command() -> Result<(), String> {
    install_mac_cli()
}