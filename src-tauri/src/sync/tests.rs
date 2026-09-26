use super::*;
use crate::config::SyncConfig;
use crate::db::Db;
use crate::types::{Entry, MediaMeta, RemoteState};
use std::path::PathBuf;

fn tempdir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xitiediary-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn fs_remote(root: &std::path::Path) -> Remote {
    let cfg = SyncConfig {
        provider: "fs".into(),
        endpoint: String::new(),
        region: String::new(),
        bucket: String::new(),
        prefix: "sync".into(),
        access_key_id: String::new(),
        access_key_secret: String::new(),
    };
    Remote::new(&cfg, root).unwrap()
}

fn client(tag: &str) -> (Db, PathBuf) {
    let dir = tempdir().join(format!("{tag}-media"));
    std::fs::create_dir_all(&dir).unwrap();
    (Db::open_in_memory().unwrap(), dir)
}

fn entry(id: &str, date: &str, content: &str, updated: i64) -> Entry {
    Entry {
        id: id.into(),
        date: date.into(),
        content: content.into(),
        created_at: updated - 10,
        updated_at: updated,
        deleted: false,
    }
}

#[tokio::test]
async fn first_sync_uploads() {
    let root = tempdir();
    let remote = fs_remote(&root);
    let (db, media_dir) = client("a");

    db.upsert_entry(&entry("e1", "2026-09-26", "第一篇", 100)).unwrap();
    let report = run_sync(&db, &remote, &media_dir, "devA").await.unwrap();
    assert_eq!(report.uploaded_entries, 1);
    assert_eq!(report.downloaded_entries, 0);

    let list = remote.list().await.unwrap();
    assert!(list.iter().any(|(k, _)| k == "entries/e1.json"));
    assert!(db.get_remote_state("entries/e1.json").unwrap().is_some());
}

#[tokio::test]
async fn bootstrap_downloads() {
    let root = tempdir();
    let remote = fs_remote(&root);

    let (db_a, media_a) = client("a");
    db_a.upsert_entry(&entry("e1", "2026-09-26", "第一篇", 100)).unwrap();
    run_sync(&db_a, &remote, &media_a, "devA").await.unwrap();

    let (db_b, media_b) = client("b");
    let report = run_sync(&db_b, &remote, &media_b, "devB").await.unwrap();
    assert_eq!(report.downloaded_entries, 1);
    assert_eq!(report.uploaded_entries, 0);

    let list = db_b.list_entries(None, None).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].content, "第一篇");
}

#[tokio::test]
async fn bidirectional_merge() {
    let root = tempdir();
    let remote = fs_remote(&root);

    let (db_a, media_a) = client("a");
    let (db_b, media_b) = client("b");
    db_a.upsert_entry(&entry("e2", "2026-09-25", "A 写的", 100)).unwrap();
    db_b.upsert_entry(&entry("e3", "2026-09-26", "B 写的", 100)).unwrap();

    run_sync(&db_a, &remote, &media_a, "devA").await.unwrap();
    run_sync(&db_b, &remote, &media_b, "devB").await.unwrap();
    run_sync(&db_a, &remote, &media_a, "devA").await.unwrap();

    let la = db_a.list_entries(None, None).unwrap();
    let lb = db_b.list_entries(None, None).unwrap();
    assert_eq!(la.len(), 2);
    assert_eq!(lb.len(), 2);
}

#[tokio::test]
async fn conflict_creates_copy_preserving_both() {
    let root = tempdir();
    let remote = fs_remote(&root);

    let (db_a, media_a) = client("a");
    let (db_b, media_b) = client("b");
    db_a.upsert_entry(&entry("e1", "2026-09-26", "原始", 100)).unwrap();
    run_sync(&db_a, &remote, &media_a, "devA").await.unwrap();
    run_sync(&db_b, &remote, &media_b, "devB").await.unwrap();

    db_a.upsert_entry(&entry("e1", "2026-09-26", "A 版本", 200)).unwrap();
    db_b.upsert_entry(&entry("e1", "2026-09-26", "B 版本", 300)).unwrap();

    run_sync(&db_a, &remote, &media_a, "devA").await.unwrap();
    let report = run_sync(&db_b, &remote, &media_b, "devB").await.unwrap();
    assert_eq!(report.conflicts, 1);

    let lb = db_b.list_entries(None, None).unwrap();
    assert_eq!(lb.len(), 2);
    let winner = lb.iter().find(|e| e.id == "e1").unwrap();
    assert_eq!(winner.content, "B 版本");
    let copy = lb.iter().find(|e| e.id != "e1").unwrap();
    assert!(copy.content.contains("[冲突"));
    assert!(copy.content.contains("A 版本"));

    run_sync(&db_a, &remote, &media_a, "devA").await.unwrap();
    let la = db_a.list_entries(None, None).unwrap();
    assert_eq!(la.len(), 2);
    assert_eq!(la.iter().find(|e| e.id == "e1").unwrap().content, "B 版本");
}

#[tokio::test]
async fn tombstone_propagates() {
    let root = tempdir();
    let remote = fs_remote(&root);

    let (db_a, media_a) = client("a");
    let (db_b, media_b) = client("b");
    db_a.upsert_entry(&entry("e1", "2026-09-26", "将被删除", 100)).unwrap();
    run_sync(&db_a, &remote, &media_a, "devA").await.unwrap();
    run_sync(&db_b, &remote, &media_b, "devB").await.unwrap();
    assert_eq!(db_b.list_entries(None, None).unwrap().len(), 1);

    db_a.soft_delete_entry("e1").unwrap();
    run_sync(&db_a, &remote, &media_a, "devA").await.unwrap();
    run_sync(&db_b, &remote, &media_b, "devB").await.unwrap();

    assert!(db_b.list_entries(None, None).unwrap().is_empty());
    assert!(db_b.get_entry("e1").unwrap().unwrap().deleted);
}

#[tokio::test]
async fn orphan_media_soft_deleted() {
    let root = tempdir();
    let remote = fs_remote(&root);

    let (db_a, media_a) = client("a");
    let (db_b, media_b) = client("b");

    db_a.upsert_entry(&entry("e1", "2026-09-26", "带图", 100)).unwrap();
    let jpeg = vec![0xff, 0xd8, 0xff, 0xe0, 1, 2, 3];
    std::fs::write(media_a.join("m1.jpg"), &jpeg).unwrap();
    db_a.insert_media(&MediaMeta {
        id: "m1".into(),
        entry_id: "e1".into(),
        mime: "image/jpeg".into(),
        size: jpeg.len() as i64,
        updated_at: 100,
        deleted: false,
    })
    .unwrap();

    run_sync(&db_a, &remote, &media_a, "devA").await.unwrap();
    let report_b = run_sync(&db_b, &remote, &media_b, "devB").await.unwrap();
    assert_eq!(report_b.downloaded_media, 1);
    assert!(media_b.join("m1.jpg").exists());
    assert_eq!(db_b.list_media("e1").unwrap().len(), 1);

    db_a.soft_delete_media("m1").unwrap();
    run_sync(&db_a, &remote, &media_a, "devA").await.unwrap();
    run_sync(&db_b, &remote, &media_b, "devB").await.unwrap();

    let all = db_b.list_all_media().unwrap();
    assert_eq!(all.len(), 1);
    assert!(all[0].deleted);
    assert!(db_b.list_media("e1").unwrap().is_empty());
}

#[tokio::test]
async fn stale_lock_is_overwritten() {
    let root = tempdir();
    let remote = fs_remote(&root);
    let (db_a, media_a) = client("a");

    remote.write_lock("devX", 1_000_000_000_000).await.unwrap();
    db_a.upsert_entry(&entry("e1", "2026-09-26", "x", 100)).unwrap();

    let report = run_sync(&db_a, &remote, &media_a, "devA").await.unwrap();
    assert_eq!(report.uploaded_entries, 1);
}

#[tokio::test]
async fn fresh_lock_skips_sync() {
    let root = tempdir();
    let remote = fs_remote(&root);
    let (db_a, media_a) = client("a");

    remote.write_lock("devX", 1_000_000_000_000_000).await.unwrap();
    db_a.upsert_entry(&entry("e1", "2026-09-26", "x", 100)).unwrap();

    let result = run_sync(&db_a, &remote, &media_a, "devA").await;
    assert!(result.is_err());
    let list = remote.list().await.unwrap();
    assert!(list.is_empty());
}

#[test]
fn apply_guard_skips_entry_edited_since_snapshot() {
    let db = Db::open_in_memory().unwrap();
    db.upsert_entry(&entry("e1", "2026-09-26", "原始", 100)).unwrap();

    // 快照值 100，当前行仍是 100（未编辑）→ 应应用远端
    assert!(should_apply_remote(&db, "e1", Some(100)));

    // 快照值 100，当前行已变为 300（同步期间用户编辑）→ 必须跳过
    db.upsert_entry(&entry("e1", "2026-09-26", "编辑中", 300)).unwrap();
    assert!(!should_apply_remote(&db, "e1", Some(100)));

    // 无本地行 + 无快照（远端-only 场景）→ 应用
    assert!(should_apply_remote(&db, "missing", None));
    // 无本地行 + 有快照（同步期间行被硬删）→ 跳过
    assert!(!should_apply_remote(&db, "missing", Some(100)));
}

#[tokio::test]
async fn corrupt_remote_entry_is_skipped() {
    let root = tempdir();
    let remote = fs_remote(&root);

    let (db_a, media_a) = client("a");
    db_a.upsert_entry(&entry("good", "2026-09-26", "好的", 100)).unwrap();
    run_sync(&db_a, &remote, &media_a, "devA").await.unwrap();

    // 直接写入一个损坏的远端条目
    remote.put_entry("bad", b"not json at all").await.unwrap();

    let (db_b, media_b) = client("b");
    let report = run_sync(&db_b, &remote, &media_b, "devB").await.unwrap();
    assert_eq!(report.skipped_objects, 1, "损坏对象应被跳过并计数");
    assert_eq!(report.downloaded_entries, 1, "好条目正常下载");
    assert!(db_b.get_entry("good").unwrap().is_some());
    assert!(db_b.get_entry("bad").unwrap().is_none());
}

#[tokio::test]
async fn garbage_lock_is_treated_as_stale() {
    let root = tempdir();
    let remote = fs_remote(&root);

    // fs provider 直接写坏锁文件（lock 不是 JSON）
    let lock_path = root.join("sync").join("lock");
    std::fs::write(&lock_path, b"garbage bytes not json").unwrap();

    let (db_a, media_a) = client("a");
    db_a.upsert_entry(&entry("e1", "2026-09-26", "x", 100)).unwrap();
    let report = run_sync(&db_a, &remote, &media_a, "devA").await.unwrap();
    assert_eq!(report.uploaded_entries, 1, "损坏锁应按陈旧处理，不阻塞");
}

#[tokio::test]
async fn conflict_copy_keeps_remote_loser_media() {
    let root = tempdir();
    let remote = fs_remote(&root);

    let (db_a, media_a) = client("a");
    let (db_b, media_b) = client("b");

    // 基线：e1 + m1 双端同步
    db_a.upsert_entry(&entry("e1", "2026-09-26", "原始", 100)).unwrap();
    add_media(&db_a, &media_a, "m1", "e1").await;
    run_sync(&db_a, &remote, &media_a, "devA").await.unwrap();
    run_sync(&db_b, &remote, &media_b, "devB").await.unwrap();

    // A 加图 m_a 并编辑（updated 200）；B 只编辑（updated 300，胜者）
    add_media(&db_a, &media_a, "m_a", "e1").await;
    db_a.upsert_entry(&entry("e1", "2026-09-26", "A 版本", 200)).unwrap();
    db_b.upsert_entry(&entry("e1", "2026-09-26", "B 版本", 300)).unwrap();

    run_sync(&db_a, &remote, &media_a, "devA").await.unwrap();
    let report = run_sync(&db_b, &remote, &media_b, "devB").await.unwrap();
    assert_eq!(report.conflicts, 1);

    let lb = db_b.list_entries(None, None).unwrap();
    assert_eq!(lb.len(), 2);
    assert_eq!(lb.iter().find(|e| e.id == "e1").unwrap().content, "B 版本");
    let copy = lb.iter().find(|e| e.id != "e1").unwrap().id.clone();

    // 败者（A 版本）独有的媒体 m_a 挂到副本名下；共享基线 m1 留在胜者名下
    let copy_media = db_b.list_media(&copy).unwrap();
    let ids: Vec<&str> = copy_media.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(copy_media.len(), 1);
    assert!(ids.contains(&"m_a"), "copy media: {ids:?}");
    assert!(media_b.join("m_a.jpg").exists(), "缺失的败者媒体应被下载");
    let winner_media = db_b.list_media("e1").unwrap();
    assert_eq!(winner_media.len(), 1);
    assert_eq!(winner_media[0].id, "m1");
}

#[tokio::test]
async fn conflict_copy_keeps_local_loser_media() {
    let root = tempdir();
    let remote = fs_remote(&root);

    let (db_a, media_a) = client("a");
    let (db_b, media_b) = client("b");

    db_a.upsert_entry(&entry("e1", "2026-09-26", "原始", 100)).unwrap();
    add_media(&db_a, &media_a, "m1", "e1").await;
    run_sync(&db_a, &remote, &media_a, "devA").await.unwrap();
    run_sync(&db_b, &remote, &media_b, "devB").await.unwrap();

    // A 编辑（200，胜者）；B 加图 m_b 并编辑（150，败者）
    db_a.upsert_entry(&entry("e1", "2026-09-26", "A 版本", 200)).unwrap();
    add_media(&db_b, &media_b, "m_b", "e1").await;
    db_b.upsert_entry(&entry("e1", "2026-09-26", "B 版本", 150)).unwrap();

    run_sync(&db_a, &remote, &media_a, "devA").await.unwrap();
    let report = run_sync(&db_b, &remote, &media_b, "devB").await.unwrap();
    assert_eq!(report.conflicts, 1);

    let lb = db_b.list_entries(None, None).unwrap();
    assert_eq!(lb.iter().find(|e| e.id == "e1").unwrap().content, "A 版本");
    let copy = lb.iter().find(|e| e.id != "e1").unwrap().id.clone();

    // 败者（B 版本）的媒体 m_b 应挂到副本名下
    let copy_media = db_b.list_media(&copy).unwrap();
    assert_eq!(copy_media.len(), 1);
    assert_eq!(copy_media[0].id, "m_b");
    assert!(media_b.join("m_b.jpg").exists());
}

async fn add_media(db: &Db, media_dir: &std::path::Path, id: &str, entry_id: &str) {
    let jpeg = vec![0xff, 0xd8, 0xff, 0xe0, 1, 2, 3, 4];
    std::fs::write(media_dir.join(format!("{id}.jpg")), &jpeg).unwrap();
    db.insert_media_quiet(&MediaMeta {
        id: id.into(),
        entry_id: entry_id.into(),
        mime: "image/jpeg".into(),
        size: jpeg.len() as i64,
        updated_at: 100,
        deleted: false,
    })
    .unwrap();
}

#[allow(dead_code)]
fn touch_remote_state() -> RemoteState {
    RemoteState {
        key: String::new(),
        etag: None,
        updated_at: None,
    }
}
