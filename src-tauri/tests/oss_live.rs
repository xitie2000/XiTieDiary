//! 真实 OSS 联通验证（手动运行，不进 CI）：
//!
//! ```powershell
//! cargo test --manifest-path src-tauri/Cargo.toml --test oss_live -- --ignored
//! ```
//!
//! 前提：仓库根目录的 local.json 已配置真实凭证。
//! 验证使用 `{prefix}-verify` 前缀，不影响正式数据；测试对象可在控制台手动清理。

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use xitiediary_lib::{load_config, run_sync, Db, Entry, MediaMeta, Remote};

fn repo_local_json() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../local.json")
}

fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "xitiediary-verify-{tag}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[tokio::test]
#[ignore = "需要真实云凭证，手动运行"]
async fn live_oss_roundtrip_two_devices() {
    let local_json = repo_local_json();
    assert!(
        local_json.is_file(),
        "未找到 {}，请先按 README 配置 local.json",
        local_json.display()
    );

    let mut cfg = load_config(Some(&local_json), None).expect("local.json 加载失败");
    cfg.prefix = format!("{}-verify", cfg.prefix);
    println!(
        "provider={} bucket={} prefix={}",
        cfg.provider, cfg.bucket, cfg.prefix
    );

    let remote = Remote::new(&cfg, &tempdir("root")).expect("远端初始化失败");

    // 设备 A：写一条带图日记并同步
    let dir_a = tempdir("a");
    let db_a = Db::open(&dir_a.join("diary.db")).unwrap();
    let now = ms();
    let entry_id = uuid::Uuid::new_v4().to_string();
    db_a.upsert_entry(&Entry {
        id: entry_id.clone(),
        date: "2026-09-26".into(),
        content: "OSS 真实同步验证".into(),
        created_at: now - 10,
        updated_at: now,
        deleted: false,
    })
    .unwrap();

    let media_id = uuid::Uuid::new_v4().to_string();
    let jpeg = vec![0xff, 0xd8, 0xff, 0xe0, 0x11, 0x22, 0x33, 0x44];
    let media_dir_a = dir_a.join("media");
    std::fs::create_dir_all(&media_dir_a).unwrap();
    std::fs::write(media_dir_a.join(format!("{media_id}.jpg")), &jpeg).unwrap();
    db_a.insert_media_quiet(&MediaMeta {
        id: media_id.clone(),
        entry_id: entry_id.clone(),
        mime: "image/jpeg".into(),
        size: jpeg.len() as i64,
        updated_at: now,
        deleted: false,
    })
    .unwrap();

    let report_a = run_sync(&db_a, &remote, &media_dir_a, "verify-a")
        .await
        .expect("设备 A 同步失败");
    println!("设备 A 报告: {report_a:?}");
    assert_eq!(report_a.uploaded_entries, 1, "应上传 1 条日记");
    assert_eq!(report_a.uploaded_media, 1, "应上传 1 张图片");

    // 设备 B：全新数据库，启动同步应拉回全部数据（Step 3 第二台设备语义）
    let dir_b = tempdir("b");
    let db_b = Db::open(&dir_b.join("diary.db")).unwrap();
    let media_dir_b = dir_b.join("media");
    std::fs::create_dir_all(&media_dir_b).unwrap();
    let report_b = run_sync(&db_b, &remote, &media_dir_b, "verify-b")
        .await
        .expect("设备 B 同步失败");
    println!("设备 B 报告: {report_b:?}");
    assert_eq!(report_b.downloaded_entries, 1, "应下载 1 条日记");
    assert_eq!(report_b.downloaded_media, 1, "应下载 1 张图片");

    let entries = db_b.list_entries(None, None).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].content, "OSS 真实同步验证");
    assert_eq!(entries[0].id, entry_id);

    let media = db_b.list_media(&entry_id).unwrap();
    assert_eq!(media.len(), 1);
    assert_eq!(media[0].id, media_id);
    let downloaded = std::fs::read(media_dir_b.join(format!("{media_id}.jpg"))).unwrap();
    assert_eq!(downloaded, jpeg, "图片二进制应逐字节一致");

    println!("✓ 真实 OSS 双设备往返验证通过");
}
