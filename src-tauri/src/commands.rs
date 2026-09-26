use crate::config::load_config;
use crate::db::{current_ms, Db};
use crate::error::{AppError, AppErrorDto};
use crate::images::compress_to_jpeg;
use crate::sync::remote::Remote;
use crate::sync::{run_sync, SyncReport};
use crate::types::{Entry, MediaMeta};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Serialize)]
pub struct ConfigStatus {
    pub configured: bool,
    pub provider: Option<String>,
    pub bucket: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MediaWithUrl {
    #[serde(flatten)]
    pub meta: MediaMeta,
    pub url_path: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct SyncStatusEvent {
    pub status: String,
    pub report: Option<SyncReport>,
    pub message: Option<String>,
}

fn device_id(data_dir: &std::path::Path) -> Result<String, AppError> {
    let path = data_dir.join("device-id");
    if let Ok(s) = std::fs::read_to_string(&path) {
        let id = s.trim();
        if !id.is_empty() {
            return Ok(id.to_string());
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    std::fs::write(&path, &id)?;
    Ok(id)
}

pub async fn do_sync_core<E>(
    data_dir: &std::path::Path,
    db: &Db,
    emit: E,
) -> Result<SyncReport, AppError>
where
    E: Fn(SyncStatusEvent),
{
    emit(SyncStatusEvent {
        status: "syncing".into(),
        report: None,
        message: None,
    });

    match sync_inner(data_dir, db).await {
        Ok(report) => {
            emit(SyncStatusEvent {
                status: "ok".into(),
                report: Some(report.clone()),
                message: None,
            });
            Ok(report)
        }
        Err(e) => {
            emit(SyncStatusEvent {
                status: "error".into(),
                report: None,
                message: Some(e.to_string()),
            });
            Err(e)
        }
    }
}

async fn sync_inner(data_dir: &std::path::Path, db: &Db) -> Result<SyncReport, AppError> {
    let cfg = load_config(None, Some(data_dir))?;
    let remote = Remote::new(&cfg, data_dir)?;
    let media_dir = data_dir.join("media");
    std::fs::create_dir_all(&media_dir)?;
    let device = device_id(data_dir)?;
    run_sync(db, &remote, &media_dir, &device).await
}

#[tauri::command]
pub async fn sync_now(app: AppHandle, db: State<'_, Db>) -> Result<SyncReport, AppErrorDto> {
    let data_dir = app_data_dir(&app)?;
    let handle = app.clone();
    do_sync_core(&data_dir, &db, move |ev| {
        let _ = handle.emit("sync://status", ev);
    })
    .await
    .map_err(AppErrorDto::from)
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

fn write_media(
    data_dir: &std::path::Path,
    db: &Db,
    entry_id: &str,
    raw: Vec<u8>,
) -> Result<MediaMeta, AppErrorDto> {
    let (bytes, _w, _h) = compress_to_jpeg(&raw, 1920, 80).map_err(dto)?;
    let media_dir = data_dir.join("media");
    std::fs::create_dir_all(&media_dir).map_err(dto)?;
    let id = uuid::Uuid::new_v4().to_string();
    std::fs::write(media_dir.join(format!("{id}.jpg")), &bytes).map_err(dto)?;
    let m = MediaMeta {
        id,
        entry_id: entry_id.to_string(),
        mime: "image/jpeg".into(),
        size: bytes.len() as i64,
        updated_at: current_ms(),
        deleted: false,
    };
    db.insert_media(&m).map_err(dto)?;
    Ok(m)
}

#[tauri::command]
pub fn insert_media(
    app: AppHandle,
    db: State<Db>,
    entry_id: String,
    path: String,
) -> Result<MediaMeta, AppErrorDto> {
    let raw = std::fs::read(&path).map_err(dto)?;
    let data_dir = app_data_dir(&app)?;
    write_media(&data_dir, &db, &entry_id, raw)
}

#[tauri::command]
pub fn insert_media_bytes(
    app: AppHandle,
    db: State<Db>,
    entry_id: String,
    data: String,
) -> Result<MediaMeta, AppErrorDto> {
    use base64::Engine;
    let raw = base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| dto(AppError::Sync(format!("图片数据解码失败: {e}"))))?;
    let data_dir = app_data_dir(&app)?;
    write_media(&data_dir, &db, &entry_id, raw)
}

#[tauri::command]
pub fn list_media(app: AppHandle, db: State<Db>, entry_id: String) -> Result<Vec<MediaWithUrl>, AppErrorDto> {
    let data_dir = app_data_dir(&app)?;
    let media_dir = data_dir.join("media");
    let list = db.list_media(&entry_id).map_err(dto)?;
    Ok(list
        .into_iter()
        .map(|meta| {
            let url_path = media_dir.join(format!("{}.jpg", meta.id)).to_string_lossy().into_owned();
            MediaWithUrl { meta, url_path }
        })
        .collect())
}

#[tauri::command]
pub fn media_counts(db: State<Db>) -> Result<std::collections::HashMap<String, u32>, AppErrorDto> {
    db.media_counts_by_entry().map_err(dto)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn test_png(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 251) as u8, (y % 241) as u8, ((x + y) % 233) as u8])
        });
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    #[test]
    fn bytes_pipeline_matches_file_pipeline() {
        let dir = std::env::temp_dir().join(format!("xitiediary-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Db::open_in_memory().unwrap();
        db.upsert_entry(&Entry {
            id: "e1".into(),
            date: "2026-09-26".into(),
            content: "x".into(),
            created_at: 1,
            updated_at: 1,
            deleted: false,
        })
        .unwrap();

        let png = test_png(300, 200);

        // 桌面路径：insert_media 读文件后调用 write_media
        let m1 = write_media(&dir, &db, "e1", png.clone()).unwrap();

        // Android 路径：base64 传输后解码，调用同一个 write_media
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(b64.as_bytes())
            .unwrap();
        let m2 = write_media(&dir, &db, "e1", decoded).unwrap();

        assert_eq!(m1.mime, m2.mime);
        assert_eq!(m1.size, m2.size);

        let f1 = std::fs::read(dir.join("media").join(format!("{}.jpg", m1.id))).unwrap();
        let f2 = std::fs::read(dir.join("media").join(format!("{}.jpg", m2.id))).unwrap();
        assert_eq!(f1, f2, "两条管线应产出逐字节一致的 JPEG");
    }

    #[tokio::test]
    async fn sync_error_emits_event() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var("XITIEDIARY_CONFIG");

        let dir = std::env::temp_dir().join(format!("xitiediary-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg_path = dir.join("local.json");
        std::fs::write(
            &cfg_path,
            r#"{"provider":"ftp","bucket":"b","access_key_id":"ak","access_key_secret":"supersecret123"}"#,
        )
        .unwrap();
        std::env::set_var("XITIEDIARY_CONFIG", &cfg_path);

        let db = Db::open_in_memory().unwrap();
        let events: Arc<Mutex<Vec<SyncStatusEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let data_dir = dir.clone();

        let result = do_sync_core(&data_dir, &db, move |ev| {
            sink.lock().unwrap().push(ev);
        })
        .await;

        std::env::remove_var("XITIEDIARY_CONFIG");

        assert!(result.is_err());
        let events = events.lock().unwrap();
        assert!(!events.is_empty(), "no events emitted");
        let last = events.last().unwrap();
        assert_eq!(last.status, "error");
        let all = format!("{events:?}");
        assert!(!all.contains("supersecret123"), "leaked secret: {all}");
    }
}
