#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    Sql(rusqlite::Error),
    Notify(notify::Error),
    Tauri(tauri::Error),
    SerdeJson(serde_json::Error),
    Message(String),
}

impl AppError {
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "IO 错误: {err}"),
            Self::Sql(err) => write!(f, "SQLite 错误: {err}"),
            Self::Notify(err) => write!(f, "文件监听错误: {err}"),
            Self::Tauri(err) => write!(f, "Tauri 错误: {err}"),
            Self::SerdeJson(err) => write!(f, "JSON 错误: {err}"),
            Self::Message(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sql(value)
    }
}

impl From<notify::Error> for AppError {
    fn from(value: notify::Error) -> Self {
        Self::Notify(value)
    }
}

impl From<tauri::Error> for AppError {
    fn from(value: tauri::Error) -> Self {
        Self::Tauri(value)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::SerdeJson(value)
    }
}

pub type AppResult<T> = Result<T, AppError>;
