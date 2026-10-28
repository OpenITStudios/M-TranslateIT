mod app_bootstrap;
mod commands;
mod engine;

use tauri::Manager;

fn restore_main_window(app_handle: &tauri::AppHandle) {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn main() {
    // Acquire before Tauri setup, path recovery, windows, workers or audio.
    #[cfg(target_os = "windows")]
    let _single_instance = match app_bootstrap::single_instance::acquire() {
        Ok(app_bootstrap::single_instance::InstanceAdmission::Primary(guard)) => guard,
        Ok(app_bootstrap::single_instance::InstanceAdmission::Existing) => return,
        Err(error) => {
            app_bootstrap::single_instance::show_startup_error(&error);
            return;
        }
    };

    let app = commands::registry::register(
        tauri::Builder::default()
            .plugin(tauri_plugin_updater::Builder::new().build())
            .setup(|app| {
                engine::runtime_events::install_app_handle(app.handle().clone());
                app_bootstrap::configure_main_window(app)
            }),
    )
    .build(tauri::generate_context!())
    .expect("TranslateIT app failed to build");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            if !commands::application_runtime::prepare_for_app_exit() {
                api.prevent_exit();
                restore_main_window(app_handle);
            }
        }
    });
}
