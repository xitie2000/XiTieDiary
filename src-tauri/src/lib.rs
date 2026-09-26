mod commands;
mod config;
mod db;
mod error;
mod images;
mod sync;
mod types;

use db::Db;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            use tauri::Manager;
            let data_dir = app.path().app_data_dir()?;
            let db = Db::open(&data_dir.join("diary.db")).map_err(std::io::Error::other)?;
            app.manage(db);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_entries,
            commands::get_entry,
            commands::create_draft_entry,
            commands::save_entry,
            commands::delete_entry,
            commands::insert_media,
            commands::delete_media,
            commands::list_media,
            commands::media_counts,
            commands::get_config_status,
            commands::cleanup_empty_drafts,
            commands::sync_now
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
