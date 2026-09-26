use crate::types::{Entry, MediaMeta};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteEntry {
    pub id: String,
    pub date: String,
    pub content: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted: bool,
}

impl From<Entry> for RemoteEntry {
    fn from(e: Entry) -> Self {
        RemoteEntry {
            id: e.id,
            date: e.date,
            content: e.content,
            created_at: e.created_at,
            updated_at: e.updated_at,
            deleted: e.deleted,
        }
    }
}

impl From<RemoteEntry> for Entry {
    fn from(e: RemoteEntry) -> Self {
        Entry {
            id: e.id,
            date: e.date,
            content: e.content,
            created_at: e.created_at,
            updated_at: e.updated_at,
            deleted: e.deleted,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntryAction {
    Ignore,
    Download,
    Upload,
    Conflict { keep_local: bool },
}

pub fn decide_entry(
    local: Option<Entry>,
    last_synced: Option<i64>,
    remote: Option<RemoteEntry>,
) -> EntryAction {
    match (local, remote) {
        (None, None) => EntryAction::Ignore,
        (Some(_), None) => EntryAction::Upload,
        (None, Some(_)) => EntryAction::Download,
        (Some(l), Some(r)) => {
            let clean = last_synced.is_some_and(|ls| l.updated_at == ls);
            if clean {
                if last_synced.is_some_and(|ls| r.updated_at > ls) {
                    EntryAction::Download
                } else {
                    EntryAction::Ignore
                }
            } else {
                let remote_changed = !last_synced.is_some_and(|ls| r.updated_at == ls);
                if !remote_changed {
                    return EntryAction::Upload;
                }
                if l.updated_at > r.updated_at {
                    EntryAction::Conflict { keep_local: true }
                } else {
                    EntryAction::Conflict { keep_local: false }
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MediaAction {
    Skip,
    Download,
    Upload,
    LocalTombstone,
}

pub fn decide_media(local: Option<&MediaMeta>, referenced: bool, on_remote: bool) -> MediaAction {
    match local {
        None => {
            if on_remote && referenced {
                MediaAction::Download
            } else {
                MediaAction::Skip
            }
        }
        Some(m) => {
            if m.deleted {
                if on_remote {
                    MediaAction::LocalTombstone
                } else {
                    MediaAction::Skip
                }
            } else if on_remote {
                MediaAction::Skip
            } else if referenced {
                MediaAction::Upload
            } else {
                MediaAction::Skip
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Entry;

    fn local(updated: i64) -> Entry {
        Entry {
            id: "e1".into(),
            date: "2026-09-26".into(),
            content: "local".into(),
            created_at: 10,
            updated_at: updated,
            deleted: false,
        }
    }

    fn remote(updated: i64) -> RemoteEntry {
        RemoteEntry {
            id: "e1".into(),
            date: "2026-09-26".into(),
            content: "remote".into(),
            created_at: 10,
            updated_at: updated,
            deleted: false,
        }
    }

    #[test]
    fn local_only_uploads() {
        let action = decide_entry(Some(local(100)), Some(100), None);
        assert_eq!(action, EntryAction::Upload);
    }

    #[test]
    fn remote_only_downloads() {
        let action = decide_entry(None, None, Some(remote(100)));
        assert_eq!(action, EntryAction::Download);
    }

    #[test]
    fn clean_local_ignores_older_remote() {
        let action = decide_entry(Some(local(100)), Some(100), Some(remote(100)));
        assert_eq!(action, EntryAction::Ignore);

        let older = decide_entry(Some(local(100)), Some(100), Some(remote(50)));
        assert_eq!(older, EntryAction::Ignore);
    }

    #[test]
    fn clean_local_downloads_newer_remote() {
        let action = decide_entry(Some(local(100)), Some(100), Some(remote(150)));
        assert_eq!(action, EntryAction::Download);
    }

    #[test]
    fn dirty_local_uploads_when_remote_unchanged() {
        let action = decide_entry(Some(local(200)), Some(100), Some(remote(100)));
        assert_eq!(action, EntryAction::Upload);
    }

    #[test]
    fn both_changed_lww_and_conflict() {
        let local_wins = decide_entry(Some(local(200)), Some(100), Some(remote(150)));
        assert_eq!(local_wins, EntryAction::Conflict { keep_local: true });

        let remote_wins = decide_entry(Some(local(150)), Some(100), Some(remote(200)));
        assert_eq!(remote_wins, EntryAction::Conflict { keep_local: false });
    }

    #[test]
    fn tombstone_json_roundtrip() {
        let mut e = remote(100);
        e.deleted = true;
        let json = serde_json::to_vec(&e).unwrap();
        let back: RemoteEntry = serde_json::from_slice(&json).unwrap();
        assert!(back.deleted);
        assert_eq!(back.updated_at, 100);
    }

    #[test]
    fn unknown_fields_ignored() {
        let json = r#"{"id":"e1","date":"2026-09-26","content":"x","created_at":1,"updated_at":2,"deleted":false,"future":1}"#;
        let e: RemoteEntry = serde_json::from_str(json).unwrap();
        assert_eq!(e.id, "e1");
        assert_eq!(e.content, "x");
    }

    #[test]
    fn media_decisions() {
        use crate::types::MediaMeta;

        fn meta(deleted: bool) -> MediaMeta {
            MediaMeta {
                id: "m1".into(),
                entry_id: "e1".into(),
                mime: "image/jpeg".into(),
                size: 1,
                updated_at: 1,
                deleted,
            }
        }

        let m = meta(false);
        let tomb = meta(true);

        assert_eq!(decide_media(None, true, true), MediaAction::Download);
        assert_eq!(decide_media(None, false, true), MediaAction::Skip);
        assert_eq!(decide_media(None, true, false), MediaAction::Skip);
        assert_eq!(decide_media(Some(&tomb), true, true), MediaAction::LocalTombstone);
        assert_eq!(decide_media(Some(&tomb), true, false), MediaAction::Skip);
        assert_eq!(decide_media(Some(&m), true, true), MediaAction::Skip);
        assert_eq!(decide_media(Some(&m), true, false), MediaAction::Upload);
        assert_eq!(decide_media(Some(&m), false, false), MediaAction::Skip);
    }
}
