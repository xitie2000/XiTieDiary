use crate::types::{Entry, MediaMeta, RemoteState};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Db {
    conn: Mutex<Connection>,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub(crate) fn current_ms() -> i64 {
    now_ms()
}

fn row_to_entry(r: &rusqlite::Row) -> rusqlite::Result<Entry> {
    Ok(Entry {
        id: r.get(0)?,
        date: r.get(1)?,
        content: r.get(2)?,
        created_at: r.get(3)?,
        updated_at: r.get(4)?,
        deleted: r.get::<_, i64>(5)? != 0,
    })
}

fn row_to_media(r: &rusqlite::Row) -> rusqlite::Result<MediaMeta> {
    Ok(MediaMeta {
        id: r.get(0)?,
        entry_id: r.get(1)?,
        mime: r.get(2)?,
        size: r.get(3)?,
        updated_at: r.get(4)?,
        deleted: r.get::<_, i64>(5)? != 0,
    })
}

fn row_to_remote_state(r: &rusqlite::Row) -> rusqlite::Result<RemoteState> {
    Ok(RemoteState {
        key: r.get(0)?,
        etag: r.get(1)?,
        updated_at: r.get(2)?,
    })
}

const ENTRY_COLS: &str = "id, date, content, created_at, updated_at, deleted";
const MEDIA_COLS: &str = "id, entry_id, mime, size, updated_at, deleted";

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS entries (
  id         TEXT PRIMARY KEY,
  date       TEXT NOT NULL,
  content    TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  deleted    INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS media (
  id         TEXT PRIMARY KEY,
  entry_id   TEXT NOT NULL,
  mime       TEXT NOT NULL,
  size       INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  deleted    INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS remote_state (
  key        TEXT PRIMARY KEY,
  etag       TEXT,
  updated_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_entries_date ON entries(date);
CREATE INDEX IF NOT EXISTS idx_media_entry  ON media(entry_id);
";

impl Db {
    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|_| rusqlite::Error::InvalidPath(dir.into()))?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> rusqlite::Result<()> {
        self.with_conn(|c| {
            c.execute_batch(SCHEMA)?;
            c.pragma_update(None, "user_version", 1)?;
            Ok(())
        })
    }

    pub(crate) fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        let conn = self.conn.lock().unwrap();
        f(&conn)
    }

    pub fn list_entries(
        &self,
        from: Option<&str>,
        to: Option<&str>,
    ) -> rusqlite::Result<Vec<Entry>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT id, date, content, created_at, updated_at, deleted FROM entries \
                 WHERE deleted = 0 \
                 AND (?1 IS NULL OR date >= ?1) \
                 AND (?2 IS NULL OR date <= ?2) \
                 ORDER BY date DESC, updated_at DESC, id ASC",
            )?;
            let rows = stmt
                .query_map(params![from, to], row_to_entry)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    pub fn get_entry(&self, id: &str) -> rusqlite::Result<Option<Entry>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {ENTRY_COLS} FROM entries WHERE id = ?1"
            ))?;
            let row = stmt.query_row(params![id], row_to_entry);
            match row {
                Ok(e) => Ok(Some(e)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    pub fn upsert_entry(&self, e: &Entry) -> rusqlite::Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO entries (id, date, content, created_at, updated_at, deleted) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT(id) DO UPDATE SET \
                 date = excluded.date, content = excluded.content, \
                 created_at = excluded.created_at, updated_at = excluded.updated_at, \
                 deleted = excluded.deleted",
                params![e.id, e.date, e.content, e.created_at, e.updated_at, e.deleted as i64],
            )?;
            Ok(())
        })
    }

    pub fn soft_delete_entry(&self, id: &str) -> rusqlite::Result<Option<Entry>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "UPDATE entries SET deleted = 1, updated_at = ?2 \
                 WHERE id = ?1 AND deleted = 0 \
                 RETURNING id, date, content, created_at, updated_at, deleted",
            )?;
            let row = stmt.query_row(params![id, now_ms()], row_to_entry);
            match row {
                Ok(e) => Ok(Some(e)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    pub fn list_all_entries(&self) -> rusqlite::Result<Vec<Entry>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {ENTRY_COLS} FROM entries ORDER BY id ASC"
            ))?;
            let rows = stmt
                .query_map([], row_to_entry)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    fn bump_entry_updated_at(&self, entry_id: &str) -> rusqlite::Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE entries SET updated_at = ?2 WHERE id = ?1 AND deleted = 0",
                params![entry_id, now_ms()],
            )?;
            Ok(())
        })
    }

    pub fn insert_media(&self, m: &MediaMeta) -> rusqlite::Result<()> {
        self.insert_media_quiet(m)?;
        self.bump_entry_updated_at(&m.entry_id)
    }

    pub fn insert_media_quiet(&self, m: &MediaMeta) -> rusqlite::Result<()> {
        let res: rusqlite::Result<()> = self.with_conn(|c| {
            c.execute(
                "INSERT OR REPLACE INTO media (id, entry_id, mime, size, updated_at, deleted) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![m.id, m.entry_id, m.mime, m.size, m.updated_at, m.deleted as i64],
            )?;
            Ok(())
        });
        res
    }

    pub fn list_media(&self, entry_id: &str) -> rusqlite::Result<Vec<MediaMeta>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {MEDIA_COLS} FROM media WHERE entry_id = ?1 AND deleted = 0"
            ))?;
            let rows = stmt
                .query_map(params![entry_id], row_to_media)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    pub fn list_all_media(&self) -> rusqlite::Result<Vec<MediaMeta>> {
        self.with_conn(|c| {
            let mut stmt =
                c.prepare(&format!("SELECT {MEDIA_COLS} FROM media ORDER BY id ASC"))?;
            let rows = stmt
                .query_map([], row_to_media)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    pub fn media_counts_by_entry(&self) -> rusqlite::Result<std::collections::HashMap<String, u32>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT entry_id, COUNT(*) FROM media WHERE deleted = 0 GROUP BY entry_id",
            )?;
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows.into_iter().collect())
        })
    }

    pub fn soft_delete_media(&self, id: &str) -> rusqlite::Result<()> {
        let entry_id = self.with_conn(|c| {
            c.query_row(
                "SELECT entry_id FROM media WHERE id = ?1",
                params![id],
                |r| r.get::<_, String>(0),
            )
        });
        if let Ok(eid) = entry_id {
            self.bump_entry_updated_at(&eid)?;
        }
        self.soft_delete_media_quiet(id)
    }

    pub fn soft_delete_media_quiet(&self, id: &str) -> rusqlite::Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE media SET deleted = 1, updated_at = ?2 WHERE id = ?1",
                params![id, now_ms()],
            )?;
            Ok(())
        })
    }

    pub fn soft_delete_media_for_entry(&self, entry_id: &str) -> rusqlite::Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE media SET deleted = 1, updated_at = ?2 WHERE entry_id = ?1",
                params![entry_id, now_ms()],
            )?;
            Ok(())
        })
    }

    pub fn cleanup_empty_drafts(&self) -> rusqlite::Result<u32> {
        self.with_conn(|c| {
            let n = c.execute(
                "DELETE FROM entries \
                 WHERE deleted = 0 AND content = '' \
                 AND id NOT IN (SELECT DISTINCT entry_id FROM media) \
                 AND id NOT IN (SELECT substr(key, 9) FROM remote_state \
                                WHERE key LIKE 'entries/%')",
                [],
            )?;
            Ok(n as u32)
        })
    }

    pub fn link_media(&self, id: &str, entry_id: &str) -> rusqlite::Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE media SET entry_id = ?2 WHERE id = ?1",
                params![id, entry_id],
            )?;
            Ok(())
        })
    }

    pub fn get_remote_state(&self, key: &str) -> rusqlite::Result<Option<RemoteState>> {
        self.with_conn(|c| {
            let mut stmt =
                c.prepare("SELECT key, etag, updated_at FROM remote_state WHERE key = ?1")?;
            let row = stmt.query_row(params![key], row_to_remote_state);
            match row {
                Ok(rs) => Ok(Some(rs)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    pub fn set_remote_state(&self, rs: &RemoteState) -> rusqlite::Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT OR REPLACE INTO remote_state (key, etag, updated_at) VALUES (?1, ?2, ?3)",
                params![rs.key, rs.etag, rs.updated_at],
            )?;
            Ok(())
        })
    }

    pub fn list_remote_state(&self) -> rusqlite::Result<Vec<RemoteState>> {
        self.with_conn(|c| {
            let mut stmt =
                c.prepare("SELECT key, etag, updated_at FROM remote_state ORDER BY key ASC")?;
            let rows = stmt
                .query_map([], row_to_remote_state)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Entry, MediaMeta, RemoteState};

    fn entry(id: &str, date: &str, content: &str) -> Entry {
        Entry {
            id: id.into(),
            date: date.into(),
            content: content.into(),
            created_at: 1000,
            updated_at: 1000,
            deleted: false,
        }
    }

    fn media(id: &str, entry_id: &str) -> MediaMeta {
        MediaMeta {
            id: id.into(),
            entry_id: entry_id.into(),
            mime: "image/jpeg".into(),
            size: 123,
            updated_at: 500,
            deleted: false,
        }
    }

    #[test]
    fn schema_creates_tables_and_indexes() {
        let db = Db::open_in_memory().unwrap();
        let version: i64 = db
            .with_conn(|c| c.query_row("PRAGMA user_version", [], |r| r.get(0)))
            .unwrap();
        assert_eq!(version, 1);
        let names: Vec<String> = db
            .with_conn(|c| {
                let mut stmt = c
                    .prepare(
                        "SELECT name FROM sqlite_master WHERE type='index' \
                         AND name IN ('idx_entries_date','idx_media_entry')",
                    )
                    .unwrap();
                stmt.query_map([], |r| r.get::<_, String>(0))
                    .unwrap()
                    .map(|r| r.unwrap())
                    .collect()
            });
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn upsert_then_list_entries_orders_by_date_desc_and_hides_deleted() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_entry(&entry("a", "2026-09-01", "one")).unwrap();
        let mut e2 = entry("b", "2026-09-02", "two");
        e2.updated_at = 2000;
        db.upsert_entry(&e2).unwrap();
        let mut e3 = entry("c", "2026-09-02", "gone");
        e3.deleted = true;
        db.upsert_entry(&e3).unwrap();

        let list = db.list_entries(None, None).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "b");
        assert_eq!(list[1].id, "a");

        let day = db.list_entries(Some("2026-09-02"), Some("2026-09-02")).unwrap();
        assert_eq!(day.len(), 1);
        assert_eq!(day[0].id, "b");
    }

    #[test]
    fn soft_delete_entry_sets_flag_and_bumps_updated_at() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_entry(&entry("a", "2026-09-01", "x")).unwrap();
        let deleted = db.soft_delete_entry("a").unwrap().unwrap();
        assert!(deleted.deleted);
        assert!(deleted.updated_at > 1000);
        assert!(db.list_entries(None, None).unwrap().is_empty());
        assert!(db.get_entry("a").unwrap().unwrap().deleted);
        assert!(db.soft_delete_entry("missing").unwrap().is_none());
    }

    #[test]
    fn media_crud_roundtrip_and_link() {
        let db = Db::open_in_memory().unwrap();
        db.insert_media(&media("m1", "e1")).unwrap();
        assert_eq!(db.list_media("e1").unwrap().len(), 1);
        assert_eq!(db.list_media("e1").unwrap()[0].size, 123);

        db.link_media("m1", "e2").unwrap();
        assert!(db.list_media("e1").unwrap().is_empty());
        assert_eq!(db.list_media("e2").unwrap().len(), 1);

        db.soft_delete_media("m1").unwrap();
        assert!(db.list_media("e2").unwrap().is_empty());
        let all = db.list_all_media().unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].deleted);

        let mut re = media("m1", "e2");
        re.size = 456;
        db.insert_media(&re).unwrap();
        assert_eq!(db.list_all_media().unwrap().len(), 1);
        assert!(!db.list_all_media().unwrap()[0].deleted);
        assert_eq!(db.list_media("e2").unwrap()[0].size, 456);
    }

    #[test]
    fn remote_state_upsert_roundtrip() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.get_remote_state("entries/x").unwrap().is_none());
        db.set_remote_state(&RemoteState {
            key: "entries/x".into(),
            etag: Some("abc".into()),
            updated_at: Some(42),
        })
        .unwrap();
        let rs = db.get_remote_state("entries/x").unwrap().unwrap();
        assert_eq!(rs.etag.as_deref(), Some("abc"));
        assert_eq!(rs.updated_at, Some(42));

        db.set_remote_state(&RemoteState {
            key: "entries/x".into(),
            etag: Some("def".into()),
            updated_at: Some(52),
        })
        .unwrap();
        let rs = db.get_remote_state("entries/x").unwrap().unwrap();
        assert_eq!(rs.etag.as_deref(), Some("def"));
        assert_eq!(db.list_remote_state().unwrap().len(), 1);
    }

    #[test]
    fn soft_delete_media_for_entry_marks_all() {
        let db = Db::open_in_memory().unwrap();
        db.insert_media(&media("m1", "e1")).unwrap();
        db.insert_media(&media("m2", "e1")).unwrap();
        db.insert_media(&media("m3", "e2")).unwrap();
        db.soft_delete_media_for_entry("e1").unwrap();
        let all = db.list_all_media().unwrap();
        assert!(all.iter().find(|m| m.id == "m1").unwrap().deleted);
        assert!(all.iter().find(|m| m.id == "m2").unwrap().deleted);
        assert!(!all.iter().find(|m| m.id == "m3").unwrap().deleted);
    }

    #[test]
    fn media_counts_by_entry_counts_only_undeleted() {
        let db = Db::open_in_memory().unwrap();
        db.insert_media(&media("m1", "e1")).unwrap();
        db.insert_media(&media("m2", "e1")).unwrap();
        db.insert_media(&media("m3", "e2")).unwrap();
        db.soft_delete_media("m3").unwrap();
        let counts = db.media_counts_by_entry().unwrap();
        assert_eq!(counts.len(), 1);
        assert_eq!(counts.get("e1"), Some(&2));
    }

    #[test]
    fn list_all_entries_includes_deleted_tombstones() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_entry(&entry("live", "2026-09-01", "x")).unwrap();
        db.upsert_entry(&entry("gone", "2026-09-01", "y")).unwrap();
        db.soft_delete_entry("gone").unwrap();
        let all = db.list_all_entries().unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().find(|e| e.id == "gone").unwrap().deleted);
        assert!(!all.iter().find(|e| e.id == "live").unwrap().deleted);
    }

    #[test]
    fn media_ops_bump_entry_updated_at() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_entry(&entry("e1", "2026-09-01", "x")).unwrap();
        let before = db.get_entry("e1").unwrap().unwrap().updated_at;
        std::thread::sleep(std::time::Duration::from_millis(5));
        db.insert_media(&media("m1", "e1")).unwrap();
        let after_insert = db.get_entry("e1").unwrap().unwrap().updated_at;
        assert!(after_insert > before);
        std::thread::sleep(std::time::Duration::from_millis(5));
        db.soft_delete_media("m1").unwrap();
        let after_delete = db.get_entry("e1").unwrap().unwrap().updated_at;
        assert!(after_delete > after_insert);
    }

    #[test]
    fn cleanup_empty_drafts_removes_only_empty_unsynced() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_entry(&entry("empty", "2026-09-01", "")).unwrap();
        db.upsert_entry(&entry("withtext", "2026-09-01", "hello")).unwrap();
        db.upsert_entry(&entry("withmedia", "2026-09-01", "")).unwrap();
        db.insert_media(&media("m1", "withmedia")).unwrap();
        db.upsert_entry(&entry("synced", "2026-09-01", "")).unwrap();
        db.set_remote_state(&RemoteState {
            key: "entries/synced".into(),
            etag: Some("abc".into()),
            updated_at: Some(1),
        })
        .unwrap();

        let removed = db.cleanup_empty_drafts().unwrap();
        assert_eq!(removed, 1);
        assert!(db.get_entry("empty").unwrap().is_none());
        assert!(db.get_entry("withtext").unwrap().is_some());
        assert!(db.get_entry("withmedia").unwrap().is_some());
        assert!(db.get_entry("synced").unwrap().is_some());
    }
}
