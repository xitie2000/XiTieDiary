use crate::config::ConfigError;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("{0}")]
    Config(#[from] ConfigError),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("同步失败: {0}")]
    Sync(String),
    #[error("未找到: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct AppErrorDto {
    pub code: String,
    pub message: String,
}

impl From<AppError> for AppErrorDto {
    fn from(e: AppError) -> Self {
        let code = match &e {
            AppError::Db(_) => "db",
            AppError::Config(_) => "config",
            AppError::Io(_) => "io",
            AppError::Sync(_) => "sync",
            AppError::NotFound(_) => "not_found",
        };
        AppErrorDto {
            code: code.into(),
            message: e.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_error_dto_codes() {
        let cases = [
            (
                AppError::Db(rusqlite::Error::QueryReturnedNoRows),
                "db",
            ),
            (
                AppError::Config(crate::config::ConfigError::Missing),
                "config",
            ),
            (
                AppError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "io boom")),
                "io",
            ),
            (AppError::Sync("sync boom".into()), "sync"),
            (AppError::NotFound("entries/abc".into()), "not_found"),
        ];
        for (err, code) in cases {
            let dto: AppErrorDto = err.into();
            assert_eq!(dto.code, code);
            assert!(!dto.message.is_empty());
        }
    }

    #[test]
    fn config_error_message_hides_secret() {
        let dir = std::env::temp_dir().join(format!("xitiediary-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("local.json");
        std::fs::write(
            &path,
            r#"{"provider":"ftp","bucket":"b","access_key_id":"ak","access_key_secret":"supersecret123"}"#,
        )
        .unwrap();
        let err = crate::config::load_config(Some(&path), None).unwrap_err();
        let app_err = AppError::Config(err);
        let text = app_err.to_string();
        assert!(!text.contains("supersecret123"), "leaked secret: {text}");
    }
}
