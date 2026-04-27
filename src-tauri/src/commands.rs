use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tauri::{AppHandle, State};

use crate::{
    config, indexer,
    models::{AddWorkspaceInput, AppConfig, FilePreviewDto, SearchResultDto, WorkspaceConfig},
    watcher::WatcherService,
};

pub struct AppState {
    pub config: Arc<Mutex<AppConfig>>,
    pub db: Arc<Mutex<Connection>>,
    pub watcher: Mutex<Option<WatcherService>>,
}

#[tauri::command]
pub fn list_workspaces(state: State<'_, AppState>) -> Result<Vec<WorkspaceConfig>, String> {
    let guard = state
        .config
        .lock()
        .map_err(|_| "配置锁已损坏".to_string())?;
    Ok(guard.workspaces.clone())
}

#[tauri::command]
pub fn add_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    input: AddWorkspaceInput,
) -> Result<WorkspaceConfig, String> {
    let workspace = {
        let mut config_guard = state
            .config
            .lock()
            .map_err(|_| "配置锁已损坏".to_string())?;
        let workspace = config::append_workspace(&mut config_guard, input).map_err(to_message)?;
        config::save_config(&app, &config_guard).map_err(to_message)?;
        workspace
    };

    let exclude_patterns = {
        let config_guard = state
            .config
            .lock()
            .map_err(|_| "配置锁已损坏".to_string())?;
        config_guard.exclude_patterns.clone()
    };
    {
        let mut db_guard = state.db.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        indexer::rebuild_workspace(&mut db_guard, &workspace, &exclude_patterns)
            .map_err(to_message)?;
    }
    {
        let mut watcher_guard = state
            .watcher
            .lock()
            .map_err(|_| "监听器锁已损坏".to_string())?;
        if let Some(watcher) = watcher_guard.as_mut() {
            watcher.refresh(&state.config).map_err(to_message)?;
        }
    }

    Ok(workspace)
}

#[tauri::command]
pub fn search(
    state: State<'_, AppState>,
    keyword: String,
    limit: Option<u32>,
) -> Result<Vec<SearchResultDto>, String> {
    let trimmed = keyword.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let db_guard = state.db.lock().map_err(|_| "数据库锁已损坏".to_string())?;
    indexer::search(&db_guard, trimmed, limit.unwrap_or(20) as usize).map_err(to_message)
}

#[tauri::command]
pub fn read_preview(state: State<'_, AppState>, path: String) -> Result<FilePreviewDto, String> {
    let db_guard = state.db.lock().map_err(|_| "数据库锁已损坏".to_string())?;
    indexer::read_preview(&db_guard, path.trim()).map_err(to_message)
}

fn to_message(error: crate::error::AppError) -> String {
    error.to_string()
}
