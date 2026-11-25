mod tray;
mod windows;

pub(crate) use windows::{show_main_window, show_quick_add_window};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri_plugin_global_shortcut::ShortcutState;

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
            if let Err(error) = crate::shortcuts::register_saved_shortcut(app.handle()) {
                eprintln!("{error}");
            }
            tray::create_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::commands::ideas::create_idea,
            crate::commands::categories::create_category,
            crate::commands::categories::rename_category,
            crate::commands::categories::delete_category,
            crate::commands::settings::get_global_shortcut,
            crate::commands::settings::set_global_shortcut,
            crate::commands::ideas::get_idea_tree,
            crate::commands::ideas::read_idea_file,
            crate::commands::ideas::save_idea_file,
            crate::commands::ideas::move_idea_file,
            crate::commands::ideas::delete_idea_file,
            crate::commands::resources::read_icon,
            crate::commands::resources::open_resource,
            crate::commands::resources::open_url,
            crate::commands::settings::get_app_settings,
            crate::commands::settings::install_mac_cli_command,
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