pub mod protocol;
pub mod remote;

use crate::db::{current_ms, Db};
use crate::error::AppError;
use crate::types::{Entry, MediaMeta, RemoteState};
use protocol::{decide_entry, decide_media, EntryAction, MediaAction, RemoteEntry};
use remote::Remote;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const LOCK_TTL_MS: i64 = 5 * 60 * 1000;

#[derive(Debug, Default, Serialize, Clone, PartialEq)]
pub struct SyncReport {
    pub downloaded_entries: u32,
    pub uploaded_entries: u32,
    pub downloaded_media: u32,
    pub uploaded_media: u32,
    pub conflicts: u32,
}

pub async fn run_sync(
    db: &Db,
    remote: &Remote,
    media_dir: &Path,
    device: &str,
) -> Result<SyncReport, AppError> {
    let now = current_ms();
    match remote.read_lock().await? {
        Some(info) if info.device != device && now.saturating_sub(info.ts) < LOCK_TTL_MS => {
            return Err(AppError::Sync(format!(
                "另一设备（{}）正在同步，请稍后重试",
                info.device
            )));
        }
        _ => {}
    }
    remote.write_lock(device, now).await?;

    let mut report = SyncReport::default();
    let result = sync_locked(db, remote, media_dir, &mut report).await;
    let _ = remote.delete_lock().await;
    result.map(|_| report)
}

async fn sync_locked(
    db: &Db,
    remote: &Remote,
    media_dir: &Path,
    report: &mut SyncReport,
) -> Result<(), AppError> {
    let remote_list: HashMap<String, Option<String>> =
        remote.list().await?.into_iter().collect();

    let local_entries = db.list_all_entries()?;
    let local_ids: HashSet<String> = local_entries.iter().map(|e| e.id.clone()).collect();

    let mut downloaded_media_refs: HashMap<String, String> = HashMap::new();
    let mut downloaded_entry_ids: HashSet<String> = HashSet::new();
    let mut pending_copies: Vec<Entry> = Vec::new();

    for e in local_entries {
        let key = format!("entries/{}.json", e.id);
        let rs = db.get_remote_state(&key)?;
        let remote_etag = remote_list.get(&key).cloned().flatten();
        let remote_present = remote_list.contains_key(&key);

        let remote_entry: Option<RemoteEntry> = if remote_present {
            let etag_known = rs
                .as_ref()
                .and_then(|r| r.etag.as_ref())
                .map(|known| remote_etag.as_deref() == Some(known.as_str()))
                .unwrap_or(false);
            if etag_known && rs.as_ref().and_then(|r| r.updated_at).is_some() {
                let ls = rs.as_ref().and_then(|r| r.updated_at).unwrap();
                Some(stub_remote(&e, ls))
            } else {
                let bytes = remote.get_entry(&e.id).await?;
                let re: RemoteEntry = serde_json::from_slice(&bytes)
                    .map_err(|err| AppError::Sync(format!("远端条目解析失败: {err}")))?;
                Some(re)
            }
        } else {
            None
        };

        let last_synced = rs.as_ref().and_then(|r| r.updated_at);
        let action = decide_entry(Some(e.clone()), last_synced, remote_entry.clone());

        match action {
            EntryAction::Ignore => {
                if remote_present {
                    db.set_remote_state(&RemoteState {
                        key: key.clone(),
                        etag: remote_etag.clone(),
                        updated_at: last_synced,
                    })?;
                }
            }
            EntryAction::Download => {
                let re = remote_entry.expect("Download requires fetched remote entry");
                db.upsert_entry(&Entry::from(re.clone()))?;
                db.set_remote_state(&RemoteState {
                    key: key.clone(),
                    etag: remote_etag.clone(),
                    updated_at: Some(re.updated_at),
                })?;
                downloaded_entry_ids.insert(re.id.clone());
                for mid in &re.media {
                    downloaded_media_refs
                        .insert(mid.clone(), re.id.clone());
                }
                report.downloaded_entries += 1;
            }
            EntryAction::Upload => {
                upload_entry(db, remote, &e, &remote_list).await?;
                report.uploaded_entries += 1;
            }
            EntryAction::Conflict { keep_local } => {
                let re = remote_entry.expect("Conflict requires fetched remote entry");
                let (winner, loser_content, loser_date) = if keep_local {
                    (e.clone(), re.content.clone(), re.date.clone())
                } else {
                    (Entry::from(re.clone()), e.content.clone(), e.date.clone())
                };
                let copy = Entry {
                    id: uuid::Uuid::new_v4().to_string(),
                    date: loser_date.clone(),
                    content: format!("[冲突 {loser_date}] {loser_content}"),
                    created_at: current_ms(),
                    updated_at: current_ms(),
                    deleted: false,
                };
                pending_copies.push(copy);
                if keep_local {
                    upload_entry(db, remote, &e, &remote_list).await?;
                } else {
                    db.upsert_entry(&winner)?;
                    downloaded_entry_ids.insert(re.id.clone());
                    for mid in &re.media {
                        downloaded_media_refs.insert(mid.clone(), re.id.clone());
                    }
                }
                db.set_remote_state(&RemoteState {
                    key: key.clone(),
                    etag: remote_etag.clone(),
                    updated_at: Some(winner_updated(&winner, keep_local, &e)),
                })?;
                report.conflicts += 1;
                report.uploaded_entries += if keep_local { 1 } else { 0 };
            }
        }
    }

    for (key, etag) in &remote_list {
        if !key.starts_with("entries/") || !key.ends_with(".json") {
            continue;
        }
        let id = key
            .trim_start_matches("entries/")
            .trim_end_matches(".json")
            .to_string();
        if local_ids.contains(&id) {
            continue;
        }
        let rs = db.get_remote_state(key)?;
        let bytes = remote.get_entry(&id).await?;
        let re: RemoteEntry = serde_json::from_slice(&bytes)
            .map_err(|err| AppError::Sync(format!("远端条目解析失败: {err}")))?;
        let action = decide_entry(None, rs.as_ref().and_then(|r| r.updated_at), Some(re.clone()));
        if matches!(action, EntryAction::Download) {
            db.upsert_entry(&Entry::from(re.clone()))?;
            db.set_remote_state(&RemoteState {
                key: key.clone(),
                etag: etag.clone(),
                updated_at: Some(re.updated_at),
            })?;
            downloaded_entry_ids.insert(re.id.clone());
            for mid in &re.media {
                downloaded_media_refs.insert(mid.clone(), re.id.clone());
            }
            report.downloaded_entries += 1;
        }
    }

    for copy in pending_copies {
        let re = RemoteEntry::from(copy.clone());
        let bytes = serde_json::to_vec(&re)
            .map_err(|e| AppError::Sync(e.to_string()))?;
        let etag = remote.put_entry(&copy.id, &bytes).await?;
        db.upsert_entry(&copy)?;
        db.set_remote_state(&RemoteState {
            key: format!("entries/{}.json", copy.id),
            etag,
            updated_at: Some(copy.updated_at),
        })?;
        report.uploaded_entries += 1;
    }

    sync_media(
        db,
        remote,
        media_dir,
        &remote_list,
        &downloaded_media_refs,
        &downloaded_entry_ids,
        report,
    )
    .await?;
    Ok(())
}

fn winner_updated(winner: &Entry, keep_local: bool, local: &Entry) -> i64 {
    if keep_local {
        local.updated_at
    } else {
        winner.updated_at
    }
}

async fn upload_entry(
    db: &Db,
    remote: &Remote,
    e: &Entry,
    remote_list: &HashMap<String, Option<String>>,
) -> Result<(), AppError> {
    let media_ids: Vec<String> = db
        .list_media(&e.id)?
        .iter()
        .map(|m| m.id.clone())
        .collect();
    let mut re = RemoteEntry::from(e.clone());
    re.media = media_ids;
    let bytes = serde_json::to_vec(&re).map_err(|err| AppError::Sync(err.to_string()))?;
    let etag = remote.put_entry(&e.id, &bytes).await?;
    let etag = match etag {
        Some(t) => Some(t),
        None => remote_list
            .get(&format!("entries/{}.json", e.id))
            .cloned()
            .flatten(),
    };
    db.set_remote_state(&RemoteState {
        key: format!("entries/{}.json", e.id),
        etag,
        updated_at: Some(e.updated_at),
    })?;
    Ok(())
}

async fn sync_media(
    db: &Db,
    remote: &Remote,
    media_dir: &Path,
    remote_list: &HashMap<String, Option<String>>,
    downloaded_refs: &HashMap<String, String>,
    downloaded_entry_ids: &HashSet<String>,
    report: &mut SyncReport,
) -> Result<(), AppError> {
    let entries_now = db.list_all_entries()?;
    let live_ids: HashSet<String> = entries_now
        .iter()
        .filter(|e| !e.deleted)
        .map(|e| e.id.clone())
        .collect();

    let mut referenced: HashMap<String, String> = HashMap::new();
    for (mid, eid) in downloaded_refs {
        referenced.insert(mid.clone(), eid.clone());
    }
    for m in db.list_all_media()? {
        if !m.deleted
            && live_ids.contains(&m.entry_id)
            && !downloaded_entry_ids.contains(&m.entry_id)
        {
            referenced.entry(m.id.clone()).or_insert_with(|| m.entry_id.clone());
        }
    }    let all_media = db.list_all_media()?;
    let mut candidates: HashSet<String> = all_media.iter().map(|m| m.id.clone()).collect();
    for key in remote_list.keys() {
        if let Some(id) = key.strip_prefix("media/") {
            candidates.insert(id.to_string());
        }
    }

    for id in candidates {
        let local_row = all_media.iter().find(|m| m.id == id);
        let on_remote = remote_list.contains_key(&format!("media/{id}"));
        let referenced_here = referenced.contains_key(&id);
        match decide_media(local_row, referenced_here, on_remote) {
            MediaAction::Download => {
                let bytes = remote.get_media(&id).await?;
                tokio::fs::write(media_dir.join(format!("{id}.jpg")), &bytes).await?;
                let entry_id = referenced
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| local_row.map(|m| m.entry_id.clone()).unwrap_or_default());
                db.insert_media_quiet(&MediaMeta {
                    id: id.clone(),
                    entry_id,
                    mime: "image/jpeg".into(),
                    size: bytes.len() as i64,
                    updated_at: current_ms(),
                    deleted: false,
                })?;
                report.downloaded_media += 1;
            }
            MediaAction::Upload => {
                let path = media_dir.join(format!("{id}.jpg"));
                if path.exists() {
                    let bytes = tokio::fs::read(&path).await?;
                    remote.put_media(&id, &bytes).await?;
                    report.uploaded_media += 1;
                }
            }
            _ => {}
        }
    }

    for m in &all_media {
        if !m.deleted && !referenced.contains_key(&m.id) {
            db.soft_delete_media_quiet(&m.id)?;
        }
    }
    Ok(())
}

fn stub_remote(e: &Entry, last_synced: i64) -> RemoteEntry {
    RemoteEntry {
        id: e.id.clone(),
        date: e.date.clone(),
        content: e.content.clone(),
        created_at: e.created_at,
        updated_at: last_synced,
        deleted: e.deleted,
        media: Vec::new(),
    }
}

#[cfg(test)]
mod tests;

