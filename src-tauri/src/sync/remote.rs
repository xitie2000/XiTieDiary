use crate::config::SyncConfig;
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Remote {
    op: opendal::Operator,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct LockInfo {
    pub device: String,
    pub ts: i64,
}

fn remote_err(e: opendal::Error) -> AppError {
    AppError::Sync(e.to_string())
}

fn system_time_ms(t: opendal::raw::Timestamp) -> i128 {
    SystemTime::from(t)
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i128)
        .unwrap_or(0)
}

impl Remote {
    pub fn new(cfg: &SyncConfig, root: &Path) -> Result<Self, AppError> {
        let op = build_operator(cfg, root)?;
        Ok(Self { op })
    }

    pub async fn list(&self) -> Result<Vec<(String, Option<String>)>, AppError> {
        let entries = self
            .op
            .list_with("/")
            .recursive(true)
            .await
            .map_err(remote_err)?;
        let mut out = Vec::new();
        for e in entries {
            let mut key = e.path().to_string();
            if key.starts_with('/') {
                key.remove(0);
            }
            if key.is_empty() || key == "lock" || key.ends_with('/') {
                continue;
            }
            let meta = e.metadata();
            let etag = match meta.etag() {
                Some(s) => Some(s.to_string()),
                None => {
                    let m = self.op.stat(e.path()).await.map_err(remote_err)?;
                    match m.etag() {
                        Some(s) => Some(s.to_string()),
                        None => {
                            let ms = m.last_modified().map(system_time_ms).unwrap_or(0);
                            Some(format!("fs:{ms}:{}", m.content_length()))
                        }
                    }
                }
            };
            out.push((key, etag));
        }
        Ok(out)
    }

    pub async fn get_entry(&self, id: &str) -> Result<Vec<u8>, AppError> {
        let buf = self
            .op
            .read(&format!("entries/{id}.json"))
            .await
            .map_err(remote_err)?;
        Ok(buf.to_vec())
    }

    pub async fn put_entry(&self, id: &str, body: &[u8]) -> Result<Option<String>, AppError> {
        let meta = self
            .op
            .write(&format!("entries/{id}.json"), body.to_vec())
            .await
            .map_err(remote_err)?;
        Ok(meta.etag().map(|s| s.to_string()))
    }

    pub async fn get_media(&self, id: &str) -> Result<Vec<u8>, AppError> {
        let buf = self.op.read(&format!("media/{id}")).await.map_err(remote_err)?;
        Ok(buf.to_vec())
    }

    pub async fn put_media(&self, id: &str, body: &[u8]) -> Result<(), AppError> {
        self.op
            .write(&format!("media/{id}"), body.to_vec())
            .await
            .map(|_| ())
            .map_err(remote_err)
    }

    pub async fn write_lock(&self, device: &str, ts: i64) -> Result<(), AppError> {
        let info = LockInfo {
            device: device.into(),
            ts,
        };
        let body = serde_json::to_vec(&info).map_err(|e| AppError::Sync(e.to_string()))?;
        self.op
            .write("lock", body)
            .await
            .map(|_| ())
            .map_err(remote_err)
    }

    pub async fn read_lock(&self) -> Result<Option<LockInfo>, AppError> {
        match self.op.read("lock").await {
            Ok(bytes) => {
                let info: LockInfo = serde_json::from_slice(&bytes.to_vec())
                    .map_err(|e| AppError::Sync(e.to_string()))?;
                Ok(Some(info))
            }
            Err(e) if e.kind() == opendal::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(remote_err(e)),
        }
    }

    pub async fn delete_lock(&self) -> Result<(), AppError> {
        match self.op.delete("lock").await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == opendal::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(remote_err(e)),
        }
    }

    pub async fn refresh_lock(&self, device: &str, ts: i64) -> Result<(), AppError> {
        self.write_lock(device, ts).await
    }
}

fn build_operator(cfg: &SyncConfig, root: &Path) -> Result<opendal::Operator, AppError> {
    let timeout = std::time::Duration::from_secs(30);
    let layer = || opendal::layers::TimeoutLayer::new().with_timeout(timeout);
    let op = match cfg.provider.as_str() {
        "fs" => {
            let dir = root.join(&cfg.prefix);
            std::fs::create_dir_all(&dir).map_err(AppError::Io)?;
            let dir_str = dir.to_string_lossy().to_string();
            let b = opendal::services::Fs::default().root(&dir_str);
            opendal::Operator::new(b).map_err(remote_err)?.layer(layer())
        }
        "oss" => {
            let mut b = opendal::services::Oss::default()
                .bucket(&cfg.bucket)
                .access_key_id(&cfg.access_key_id)
                .access_key_secret(&cfg.access_key_secret);
            if !cfg.endpoint.is_empty() {
                b = b.endpoint(&cfg.endpoint);
            }
            if !cfg.prefix.is_empty() {
                b = b.root(&cfg.prefix);
            }
            opendal::Operator::new(b).map_err(remote_err)?.layer(layer())
        }
        "s3" => {
            let mut b = opendal::services::S3::default()
                .bucket(&cfg.bucket)
                .access_key_id(&cfg.access_key_id)
                .secret_access_key(&cfg.access_key_secret);
            if !cfg.endpoint.is_empty() {
                b = b.endpoint(&cfg.endpoint);
            }
            if !cfg.region.is_empty() {
                b = b.region(&cfg.region);
            }
            if !cfg.prefix.is_empty() {
                b = b.root(&cfg.prefix);
            }
            opendal::Operator::new(b).map_err(remote_err)?.layer(layer())
        }
        "cos" => {
            let mut b = opendal::services::Cos::default()
                .bucket(&cfg.bucket)
                .secret_id(&cfg.access_key_id)
                .secret_key(&cfg.access_key_secret);
            if !cfg.endpoint.is_empty() {
                b = b.endpoint(&cfg.endpoint);
            }
            if !cfg.prefix.is_empty() {
                b = b.root(&cfg.prefix);
            }
            opendal::Operator::new(b).map_err(remote_err)?.layer(layer())
        }
        "webdav" => {
            let mut b = opendal::services::Webdav::default()
                .endpoint(&cfg.endpoint)
                .username(&cfg.access_key_id)
                .password(&cfg.access_key_secret);
            if !cfg.prefix.is_empty() {
                b = b.root(&cfg.prefix);
            }
            opendal::Operator::new(b).map_err(remote_err)?.layer(layer())
        }
        other => return Err(AppError::Sync(format!("不支持的 provider: {other}"))),
    };
    Ok(op)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fs_remote(dir: &Path) -> Remote {
        let cfg = SyncConfig {
            provider: "fs".into(),
            endpoint: String::new(),
            region: String::new(),
            bucket: String::new(),
            prefix: "sync-test".into(),
            access_key_id: String::new(),
            access_key_secret: String::new(),
        };
        Remote::new(&cfg, dir).unwrap()
    }

    fn tempdir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("xitiediary-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn fs_roundtrip_put_list_get() {
        let dir = tempdir();
        let remote = fs_remote(&dir);

        remote.put_entry("abc", b"{\"id\":\"abc\"}").await.unwrap();
        remote.put_media("m1", b"\xff\xd8fakejpeg").await.unwrap();

        let list = remote.list().await.unwrap();
        let keys: Vec<&str> = list.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"entries/abc.json"));
        assert!(keys.contains(&"media/m1"));

        let entry = remote.get_entry("abc").await.unwrap();
        assert_eq!(entry.as_slice(), b"{\"id\":\"abc\"}");
        let media = remote.get_media("m1").await.unwrap();
        assert_eq!(media.as_slice(), b"\xff\xd8fakejpeg");
    }

    #[tokio::test]
    async fn lock_write_read_delete() {
        let dir = tempdir();
        let remote = fs_remote(&dir);

        assert!(remote.read_lock().await.unwrap().is_none());
        remote.write_lock("dev1", 1234).await.unwrap();
        let info = remote.read_lock().await.unwrap().unwrap();
        assert_eq!(info.device, "dev1");
        assert_eq!(info.ts, 1234);
        remote.delete_lock().await.unwrap();
        assert!(remote.read_lock().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn fs_etag_changes_on_overwrite() {
        let dir = tempdir();
        let remote = fs_remote(&dir);

        remote.put_entry("abc", b"1").await.unwrap();
        let etag1 = remote
            .list()
            .await
            .unwrap()
            .into_iter()
            .find(|(k, _)| k == "entries/abc.json")
            .unwrap()
            .1
            .unwrap();

        remote.put_entry("abc", b"22").await.unwrap();
        let etag2 = remote
            .list()
            .await
            .unwrap()
            .into_iter()
            .find(|(k, _)| k == "entries/abc.json")
            .unwrap()
            .1
            .unwrap();

        assert_ne!(etag1, etag2);
    }
}

