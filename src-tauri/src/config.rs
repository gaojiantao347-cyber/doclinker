use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::{path::BaseDirectory, AppHandle, Manager};

use crate::{
    error::{AppError, AppResult},
    models::{AddWorkspaceInput, AppConfig, WorkspaceConfig},
};

const APP_DIR_NAME: &str = "DocLinker";
const CONFIG_FILE_NAME: &str = "config.json";
const DB_FILE_NAME: &str = "doclinker.db";

pub fn app_data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let path = app
        .path()
        .resolve(APP_DIR_NAME, BaseDirectory::AppData)
        .map_err(AppError::from)?;
    fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn config_file_path(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(app_data_dir(app)?.join(CONFIG_FILE_NAME))
}

pub fn database_file_path(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(app_data_dir(app)?.join(DB_FILE_NAME))
}

pub fn load_or_create_config(app: &AppHandle) -> AppResult<AppConfig> {
    let path = config_file_path(app)?;
    if !path.exists() {
        let config = AppConfig::default();
        save_config(app, &config)?;
        return Ok(config);
    }

    let content = fs::read_to_string(&path)?;
    let config = serde_json::from_str::<AppConfig>(&content)?;
    Ok(config)
}

pub fn save_config(app: &AppHandle, config: &AppConfig) -> AppResult<()> {
    let path = config_file_path(app)?;
    save_config_to_path(&path, config)
}

fn save_config_to_path(path: &Path, config: &AppConfig) -> AppResult<()> {
    let temp_path = path.with_extension("json.tmp");
    let content = serde_json::to_string_pretty(config)?;
    fs::write(&temp_path, content)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temp_path, path)?;
    Ok(())
}

pub fn normalize_workspace_path(path: &str) -> AppResult<PathBuf> {
    let input = PathBuf::from(path);
    if !input.exists() {
        return Err(AppError::message(format!("工作空间路径不存在: {path}")));
    }
    if !input.is_dir() {
        return Err(AppError::message(format!("工作空间路径不是目录: {path}")));
    }

    Ok(fs::canonicalize(input)?)
}

pub fn append_workspace(
    config: &mut AppConfig,
    input: AddWorkspaceInput,
) -> AppResult<WorkspaceConfig> {
    let normalized_path = normalize_workspace_path(&input.path)?;
    let normalized_string = normalized_path.to_string_lossy().to_string();

    if config
        .workspaces
        .iter()
        .any(|workspace| workspace.path.eq_ignore_ascii_case(&normalized_string))
    {
        return Err(AppError::message("该工作空间已存在"));
    }

    let name = input
        .name
        .unwrap_or_else(|| infer_workspace_name(&normalized_path));

    let workspace = WorkspaceConfig {
        id: generate_workspace_id(),
        name,
        path: normalized_string,
        kind: input.kind,
        enabled: true,
    };

    config.workspaces.push(workspace.clone());
    Ok(workspace)
}

fn infer_workspace_name(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(str::to_owned)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

fn generate_workspace_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("ws-{millis}")
}
