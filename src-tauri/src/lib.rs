mod activation;
mod commands;
mod database;
mod downloader;
mod editor;
mod filesystem;
mod models;
mod queue;
mod settings;
mod transcribe;

use tauri::Manager;

use database::Db;
use queue::QueueManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());

    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init());
    }

    builder
        .setup(|app| {
            let handle = app.handle();

            let app_data = handle.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data)?;

            let default_download_path = settings::default_download_dir(handle);

            let db = Db::init(&app_data.join("app.db"), &default_download_path)?;
            db.reset_in_flight()?;
            app.manage(db);

            let queue_manager = QueueManager::new();
            queue_manager.start(handle.clone());
            app.manage(queue_manager);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::fetch_metadata,
            commands::check_duplicate,
            commands::start_download,
            commands::pause_download,
            commands::resume_download,
            commands::cancel_download,
            commands::retry_download,
            commands::remove_download,
            commands::clear_history,
            commands::reorder_queue,
            commands::list_downloads,
            commands::search_library,
            commands::list_platforms,
            commands::get_settings,
            commands::update_settings,
            commands::open_file,
            commands::show_in_folder,
            commands::open_download_folder,
            commands::get_activation,
            commands::activate_pro,
            commands::deactivate_pro,
            commands::run_edit,
            commands::transcribe,
            commands::extract_audio,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
