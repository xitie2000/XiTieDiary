use crate::config::load_config;
use crate::db::{current_ms, Db};
use crate::error::{AppError, AppErrorDto};
use crate::types::{Entry, MediaMeta};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

#[derive(Debug, Serialize)]
pub struct ConfigStatus {
    pub configured: bool,
    pub provider: Option<String>,
    pub bucket: Option<String>,
}

fn dto(e: impl Into<AppError>) -> AppErrorDto {
    let app: AppError = e.into();
    app.into()
}

fn app_data_dir(app: &AppHandle) -> Result<std::path::PathBuf, AppErrorDto> {
    app.path()
        .app_data_dir()
        .map_err(|e| dto(AppError::Io(std::io::Error::other(e.to_string()))))
}

#[tauri::command]
pub fn list_entries(
    db: State<Db>,
    from: Option<String>,
    to: Option<String>,
) -> Result<Vec<Entry>, AppErrorDto> {
    db.list_entries(from.as_deref(), to.as_deref()).map_err(dto)
}

#[tauri::command]
pub fn get_entry(db: State<Db>, id: String) -> Result<Option<Entry>, AppErrorDto> {
    db.get_entry(&id).map_err(dto)
}

#[tauri::command]
pub fn create_draft_entry(db: State<Db>, date: String) -> Result<Entry, AppErrorDto> {
    let now = current_ms();
    let e = Entry {
        id: uuid::Uuid::new_v4().to_string(),
        date,
        content: String::new(),
        created_at: now,
        updated_at: now,
        deleted: false,
    };
    db.upsert_entry(&e).map_err(dto)?;
    Ok(e)
}

#[tauri::command]
pub fn save_entry(
    db: State<Db>,
    id: String,
    date: String,
    content: String,
) -> Result<Entry, AppErrorDto> {
    let mut e = db
        .get_entry(&id)
        .map_err(dto)?
        .ok_or_else(|| dto(AppError::NotFound(format!("entries/{id}"))))?;
    e.date = date;
    e.content = content;
    e.updated_at = current_ms();
    db.upsert_entry(&e).map_err(dto)?;
    Ok(e)
}

#[tauri::command]
pub fn delete_entry(db: State<Db>, id: String) -> Result<(), AppErrorDto> {
    db.soft_delete_entry(&id)
        .map_err(dto)?
        .ok_or_else(|| dto(AppError::NotFound(format!("entries/{id}"))))?;
    db.soft_delete_media_for_entry(&id).map_err(dto)?;
    Ok(())
}

#[tauri::command]
pub fn insert_media(
    app: AppHandle,
    db: State<Db>,
    entry_id: String,
    path: String,
) -> Result<MediaMeta, AppErrorDto> {
    let bytes = std::fs::read(&path).map_err(dto)?;
    let data_dir = app_data_dir(&app)?;
    let media_dir = data_dir.join("media");
    std::fs::create_dir_all(&media_dir).map_err(dto)?;
    let id = uuid::Uuid::new_v4().to_string();
    std::fs::write(media_dir.join(format!("{id}.jpg")), &bytes).map_err(dto)?;
    let ext = std::path::Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/jpeg",
    }
    .to_string();
    let m = MediaMeta {
        id,
        entry_id,
        mime,
        size: bytes.len() as i64,
        updated_at: current_ms(),
        deleted: false,
    };
    db.insert_media(&m).map_err(dto)?;
    Ok(m)
}

#[tauri::command]
pub fn delete_media(db: State<Db>, id: String) -> Result<(), AppErrorDto> {
    db.soft_delete_media(&id).map_err(dto)
}

#[tauri::command]
pub fn get_config_status(app: AppHandle) -> Result<ConfigStatus, AppErrorDto> {
    let data_dir = app_data_dir(&app)?;
    match load_config(None, Some(&data_dir)) {
        Ok(cfg) => Ok(ConfigStatus {
            configured: true,
            provider: Some(cfg.provider),
            bucket: Some(cfg.bucket),
        }),
        Err(_) => Ok(ConfigStatus {
            configured: false,
            provider: None,
            bucket: None,
        }),
    }
}

#[tauri::command]
pub fn cleanup_empty_drafts(db: State<Db>) -> Result<u32, AppErrorDto> {
    db.cleanup_empty_drafts().map_err(dto)
}
