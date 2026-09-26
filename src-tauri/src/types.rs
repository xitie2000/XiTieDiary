use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub id: String,
    pub date: String,
    pub content: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaMeta {
    pub id: String,
    pub entry_id: String,
    pub mime: String,
    pub size: i64,
    pub updated_at: i64,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteState {
    pub key: String,
    pub etag: Option<String>,
    pub updated_at: Option<i64>,
}
