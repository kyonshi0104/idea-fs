use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::config::read_app_config;

pub(crate) fn register_saved_shortcut(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(shortcut) = read_app_config(app)?.global_shortcut() {
        app.global_shortcut()
            .register(shortcut.as_str())
            .map_err(|error| format!("Failed to register global shortcut `{shortcut}`: {error}"))?;
    }
    Ok(())
}

pub(crate) fn restore_shortcut(
    app: &tauri::AppHandle,
    shortcut: Option<&str>,
) -> Result<(), String> {
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
