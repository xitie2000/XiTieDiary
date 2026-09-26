use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

const DEFAULT_PREFIX: &str = "xitiediary";
const VALID_PROVIDERS: [&str; 4] = ["oss", "s3", "cos", "webdav"];

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct SyncConfig {
    pub provider: String,
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub prefix: String,
    pub access_key_id: String,
    pub access_key_secret: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigError {
    Missing,
    Malformed(String),
    Incomplete(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Missing => write!(f, "未找到同步配置 local.json"),
            ConfigError::Malformed(m) => write!(f, "local.json 解析失败: {m}"),
            ConfigError::Incomplete(field) => write!(f, "配置不完整: 缺少 {field}"),
        }
    }
}

impl std::error::Error for ConfigError {}

fn env_override(field: &mut String, var: &str) {
    if let Ok(v) = std::env::var(var) {
        if !v.is_empty() {
            *field = v;
        }
    }
}

fn apply_env_overrides(cfg: &mut SyncConfig) {
    env_override(&mut cfg.provider, "XITIEDIARY_PROVIDER");
    env_override(&mut cfg.endpoint, "XITIEDIARY_ENDPOINT");
    env_override(&mut cfg.region, "XITIEDIARY_REGION");
    env_override(&mut cfg.bucket, "XITIEDIARY_BUCKET");
    env_override(&mut cfg.prefix, "XITIEDIARY_PREFIX");
    env_override(&mut cfg.access_key_id, "XITIEDIARY_ACCESS_KEY_ID");
    env_override(&mut cfg.access_key_secret, "XITIEDIARY_ACCESS_KEY_SECRET");
}

pub(crate) fn validate_config(mut cfg: SyncConfig) -> Result<SyncConfig, ConfigError> {
    if cfg.provider.is_empty() || !VALID_PROVIDERS.contains(&cfg.provider.as_str()) {
        return Err(ConfigError::Incomplete("provider".into()));
    }
    if cfg.bucket.is_empty() {
        return Err(ConfigError::Incomplete("bucket".into()));
    }
    if cfg.access_key_id.is_empty() {
        return Err(ConfigError::Incomplete("access_key_id".into()));
    }
    if cfg.access_key_secret.is_empty() {
        return Err(ConfigError::Incomplete("access_key_secret".into()));
    }
    if cfg.prefix.is_empty() {
        cfg.prefix = DEFAULT_PREFIX.into();
    }
    Ok(cfg)
}

pub fn load_config(
    explicit: Option<&Path>,
    app_data: Option<&Path>,
) -> Result<SyncConfig, ConfigError> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(p) = explicit {
        candidates.push(p.to_path_buf());
    }
    if let Ok(p) = std::env::var("XITIEDIARY_CONFIG") {
        if !p.is_empty() {
            candidates.push(PathBuf::from(p));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("local.json"));
    }
    if let Some(d) = app_data {
        candidates.push(d.join("local.json"));
    }
    let path = candidates
        .into_iter()
        .find(|p| p.is_file())
        .ok_or(ConfigError::Missing)?;
    let text = std::fs::read_to_string(&path).map_err(|e| ConfigError::Malformed(e.to_string()))?;
    let cfg: SyncConfig = serde_json::from_str(&text)
        .map_err(|e| ConfigError::Malformed(e.to_string()))?;
    let mut cfg = cfg;
    apply_env_overrides(&mut cfg);
    validate_config(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xitiediary-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_local_json(dir: &PathBuf, json: &str) -> PathBuf {
        let path = dir.join("local.json");
        fs::write(&path, json).unwrap();
        path
    }

    const FULL_JSON: &str = r#"{
        "provider": "oss",
        "endpoint": "",
        "region": "cn-hangzhou",
        "bucket": "file-bucket",
        "prefix": "",
        "access_key_id": "ak",
        "access_key_secret": "sk"
    }"#;

    #[test]
    fn missing_config_is_error() {
        let _g = LOCK.lock().unwrap();
        std::env::remove_var("XITIEDIARY_CONFIG");
        let dir = tempdir();
        let old_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let err = load_config(None, Some(&dir.join("nope.json"))).unwrap_err();
        std::env::set_current_dir(old_cwd).unwrap();
        assert_eq!(err, ConfigError::Missing);
    }

    #[test]
    fn parses_local_json() {
        let _g = LOCK.lock().unwrap();
        std::env::remove_var("XITIEDIARY_CONFIG");
        std::env::remove_var("XITIEDIARY_BUCKET");
        let dir = tempdir();
        write_local_json(&dir, FULL_JSON);
        let old_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let cfg = load_config(None, None).unwrap();
        std::env::set_current_dir(old_cwd).unwrap();
        assert_eq!(cfg.provider, "oss");
        assert_eq!(cfg.bucket, "file-bucket");
        assert_eq!(cfg.prefix, "xitiediary");
        assert_eq!(cfg.access_key_secret, "sk");
    }

    #[test]
    fn env_overrides_json() {
        let _g = LOCK.lock().unwrap();
        std::env::remove_var("XITIEDIARY_CONFIG");
        std::env::set_var("XITIEDIARY_BUCKET", "env-bucket");
        let dir = tempdir();
        let path = write_local_json(&dir, FULL_JSON);
        let cfg = load_config(Some(&path), None).unwrap();
        std::env::remove_var("XITIEDIARY_BUCKET");
        assert_eq!(cfg.bucket, "env-bucket");
    }

    #[test]
    fn empty_bucket_is_incomplete() {
        let _g = LOCK.lock().unwrap();
        std::env::remove_var("XITIEDIARY_CONFIG");
        std::env::remove_var("XITIEDIARY_BUCKET");
        let dir = tempdir();
        let path = write_local_json(
            &dir,
            r#"{"provider":"oss","bucket":"","access_key_id":"ak","access_key_secret":"sk"}"#,
        );
        let err = load_config(Some(&path), None).unwrap_err();
        assert_eq!(err, ConfigError::Incomplete("bucket".into()));
    }

    #[test]
    fn invalid_provider_is_incomplete() {
        let _g = LOCK.lock().unwrap();
        std::env::remove_var("XITIEDIARY_CONFIG");
        std::env::remove_var("XITIEDIARY_PROVIDER");
        let dir = tempdir();
        let path = write_local_json(
            &dir,
            r#"{"provider":"ftp","bucket":"b","access_key_id":"ak","access_key_secret":"sk"}"#,
        );
        let err = load_config(Some(&path), None).unwrap_err();
        assert_eq!(err, ConfigError::Incomplete("provider".into()));
    }

    #[test]
    fn malformed_json_is_error() {
        let _g = LOCK.lock().unwrap();
        std::env::remove_var("XITIEDIARY_CONFIG");
        let dir = tempdir();
        let path = write_local_json(&dir, "not json at all");
        let err = load_config(Some(&path), None).unwrap_err();
        assert!(matches!(err, ConfigError::Malformed(_)));
    }
}
