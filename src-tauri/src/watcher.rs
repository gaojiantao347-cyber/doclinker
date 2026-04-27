use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use notify::{
    event::{ModifyKind, RenameMode},
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use rusqlite::Connection;

use crate::{
    error::AppResult,
    indexer,
    models::{AppConfig, WorkspaceConfig},
};

pub struct WatcherService {
    watcher: RecommendedWatcher,
    watched_paths: HashSet<PathBuf>,
}

impl WatcherService {
    pub fn start(config: Arc<Mutex<AppConfig>>, db: Arc<Mutex<Connection>>) -> AppResult<Self> {
        let (tx, rx) = mpsc::channel();
        let watcher_config = Config::default();
        let watcher = RecommendedWatcher::new(tx, watcher_config)?;

        let config_ref = Arc::clone(&config);
        let db_ref = Arc::clone(&db);
        thread::spawn(move || event_loop(rx, config_ref, db_ref));

        let mut service = Self {
            watcher,
            watched_paths: HashSet::new(),
        };
        service.refresh(&config)?;
        Ok(service)
    }

    pub fn refresh(&mut self, config: &Arc<Mutex<AppConfig>>) -> AppResult<()> {
        let guard = config.lock().expect("config lock poisoned");
        let desired_paths: HashSet<PathBuf> = guard
            .workspaces
            .iter()
            .filter(|workspace| workspace.enabled)
            .map(|workspace| PathBuf::from(&workspace.path))
            .collect();

        for path in self.watched_paths.drain() {
            self.watcher.unwatch(&path)?;
        }

        for path in &desired_paths {
            self.watcher.watch(path, RecursiveMode::Recursive)?;
        }

        self.watched_paths = desired_paths;
        Ok(())
    }
}

fn event_loop(
    rx: mpsc::Receiver<notify::Result<Event>>,
    config: Arc<Mutex<AppConfig>>,
    db: Arc<Mutex<Connection>>,
) {
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
    let debounce = Duration::from_millis(500);

    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(event)) => handle_raw_event(event, &config, &db, &mut pending),
            Ok(Err(err)) => log::warn!("文件监听错误: {err}"),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                flush_pending(&config, &db, &mut pending, debounce)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn handle_raw_event(
    event: Event,
    config: &Arc<Mutex<AppConfig>>,
    db: &Arc<Mutex<Connection>>,
    pending: &mut HashMap<PathBuf, Instant>,
) {
    match &event.kind {
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if event.paths.len() >= 2 => {
            let old_path = event.paths[0].clone();
            let new_path = event.paths[1].clone();
            if let Some(workspace) = find_workspace_for_path(config, &new_path)
                .or_else(|| find_workspace_for_path(config, &old_path))
            {
                if let Ok(mut conn) = db.lock() {
                    if let Err(err) =
                        indexer::rename_path(&mut conn, &workspace, &old_path, &new_path)
                    {
                        log::warn!("处理重命名失败: {err}");
                    }
                }
            }
        }
        EventKind::Remove(_) => {
            for path in event.paths {
                if let Ok(mut conn) = db.lock() {
                    if let Err(err) = indexer::remove_path(&mut conn, &path) {
                        log::warn!("删除索引失败: {err}");
                    }
                }
            }
        }
        kind if kind.is_create() || kind.is_modify() => {
            for path in event.paths {
                pending.insert(path, Instant::now());
            }
        }
        _ => {}
    }
}

fn flush_pending(
    config: &Arc<Mutex<AppConfig>>,
    db: &Arc<Mutex<Connection>>,
    pending: &mut HashMap<PathBuf, Instant>,
    debounce: Duration,
) {
    let now = Instant::now();
    let ready_paths: Vec<PathBuf> = pending
        .iter()
        .filter_map(|(path, at)| {
            if now.duration_since(*at) >= debounce {
                Some(path.clone())
            } else {
                None
            }
        })
        .collect();

    for path in ready_paths {
        pending.remove(&path);
        if path.is_dir() {
            continue;
        }
        let Some(workspace) = find_workspace_for_path(config, &path) else {
            continue;
        };
        if let Ok(mut conn) = db.lock() {
            if let Err(err) = indexer::index_path(&mut conn, &workspace, &path) {
                if let Err(remove_err) = indexer::remove_path(&mut conn, &path) {
                    log::warn!("增量索引失败且删除旧索引失败: {err}; {remove_err}");
                } else {
                    log::warn!("增量索引失败，已删除旧索引: {err}");
                }
            }
        }
    }
}

fn find_workspace_for_path(config: &Arc<Mutex<AppConfig>>, path: &Path) -> Option<WorkspaceConfig> {
    let guard = config.lock().ok()?;
    let normalized = path.to_string_lossy();

    guard
        .workspaces
        .iter()
        .filter(|workspace| workspace.enabled)
        .find(|workspace| normalized.starts_with(&workspace.path))
        .cloned()
}
