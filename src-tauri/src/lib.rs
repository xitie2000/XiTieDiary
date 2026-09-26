mod commands;
mod config;
mod db;
mod error;
mod images;
mod sync;
mod types;

pub use config::load_config;
pub use db::Db;
pub use sync::remote::Remote;
pub use sync::run_sync;
pub use types::{Entry, MediaMeta};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            use tauri::Manager;
            let data_dir = app.path().app_data_dir()?;
            let db = Db::open(&data_dir.join("diary.db")).map_err(std::io::Error::other)?;
            app.manage(std::sync::Arc::new(db));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_entries,
            commands::get_entry,
            commands::create_draft_entry,
            commands::save_entry,
            commands::delete_entry,
            commands::insert_media,
            commands::insert_media_bytes,
            commands::delete_media,
            commands::list_media,
            commands::media_counts,
            commands::get_config_status,
            commands::save_config,
            commands::import_config_from_path,
            commands::cleanup_empty_drafts,
            commands::sync_now
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Android：rustls-platform-verifier 需要在任何 TLS 校验发生前完成 JNI 初始化，
/// 否则首次 HTTPS 请求会 abort（"Expect rustls-platform-verifier to be initialized"）。
/// 在库加载（System.loadLibrary）时通过 JNI_OnLoad 挂接。
#[cfg(target_os = "android")]
mod android_init {
    use jni::sys::{jint, JNI_VERSION_1_6};

    #[no_mangle]
    pub extern "system" fn JNI_OnLoad(
        vm: *mut jni::sys::JavaVM,
        _reserved: *mut std::ffi::c_void,
    ) -> jint {
        if !vm.is_null() {
            let vm = unsafe { jni::JavaVM::from_raw(vm) };
            let _ = vm.attach_current_thread_for_scope(
                |env| -> Result<(), jni::errors::Error> {
                    let class =
                        env.find_class(jni::jni_str!("android/app/ActivityThread"))?;
                    let app = env
                        .call_static_method(
                            class,
                            jni::jni_str!("currentApplication"),
                            jni::jni_sig!("()Landroid/app/Application;"),
                            &[],
                        )?
                        .l()?;
                    let _ = rustls_platform_verifier::android::init_with_env(env, app);
                    Ok(())
                },
            );
        }
        JNI_VERSION_1_6
    }
}
