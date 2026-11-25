use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

pub(crate) fn show_main_window(app: &tauri::AppHandle) -> Result<(), String> {
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

pub(crate) fn show_quick_add_window(app: &tauri::AppHandle) -> Result<(), String> {
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