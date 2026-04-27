mod commands;
mod config;
mod db;
mod error;
mod indexer;
mod models;
mod scanner;
mod watcher;

use std::sync::{Arc, Mutex};

use commands::{add_workspace, list_workspaces, read_preview, search, AppState};
use tauri::{
    menu::{CheckMenuItemBuilder, MenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, RunEvent, WindowEvent,
};

const TRAY_ID: &str = "doclinker-tray";
const TRAY_MENU_TOGGLE_WINDOW: &str = "toggle_window";
const TRAY_MENU_LAUNCH_ON_STARTUP: &str = "launch_on_startup";
const TRAY_MENU_QUIT: &str = "quit";

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn hide_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        match window.is_visible() {
            Ok(true) => {
                let _ = window.hide();
            }
            Ok(false) | Err(_) => {
                show_main_window(app);
            }
        }
    }
}

fn is_launch_on_startup_enabled(app: &tauri::AppHandle, fallback: bool) -> bool {
    use tauri_plugin_autostart::ManagerExt;

    match app.autolaunch().is_enabled() {
        Ok(enabled) => enabled,
        Err(err) => {
            log::warn!("读取开机自启动状态失败: {err}");
            fallback
        }
    }
}

fn set_launch_on_startup(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;

    let autostart = app.autolaunch();
    let result = if enabled {
        autostart.enable()
    } else {
        autostart.disable()
    };

    result.map_err(|err| err.to_string())
}

fn save_launch_on_startup_config(app: &tauri::AppHandle, enabled: bool) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };

    let mut config_guard = match state.config.lock() {
        Ok(guard) => guard,
        Err(_) => {
            log::warn!("保存开机自启动配置失败: 配置锁已损坏");
            return;
        }
    };

    if config_guard.launch_on_startup == enabled {
        return;
    }

    config_guard.launch_on_startup = enabled;
    if let Err(err) = config::save_config(app, &config_guard) {
        log::warn!("保存开机自启动配置失败: {err}");
    }
}

fn create_tray(app: &mut tauri::App, launch_on_startup: bool) -> tauri::Result<()> {
    let launch_on_startup_item =
        CheckMenuItemBuilder::with_id(TRAY_MENU_LAUNCH_ON_STARTUP, "开机自动启动")
            .checked(launch_on_startup)
            .build(app)?;

    let menu = MenuBuilder::new(app)
        .text(TRAY_MENU_TOGGLE_WINDOW, "显示/隐藏 DocLinker")
        .item(&launch_on_startup_item)
        .separator()
        .text(TRAY_MENU_QUIT, "退出")
        .build()?;

    let launch_on_startup_item = launch_on_startup_item.clone();
    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("DocLinker")
        .on_menu_event(move |app, event| match event.id().as_ref() {
            TRAY_MENU_TOGGLE_WINDOW => toggle_main_window(app),
            TRAY_MENU_LAUNCH_ON_STARTUP => {
                let enabled = launch_on_startup_item.is_checked().unwrap_or(false);
                if let Err(err) = set_launch_on_startup(app, enabled) {
                    log::warn!("设置开机自启动失败: {err}");
                    let _ = launch_on_startup_item.set_checked(!enabled);
                    return;
                }
                save_launch_on_startup_config(app, enabled);
            }
            TRAY_MENU_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }

    tray.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = env_logger::try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

                    let alt_f = Shortcut::new(Some(Modifiers::ALT), Code::KeyF);

                    if shortcut == &alt_f && event.state() == ShortcutState::Pressed {
                        toggle_main_window(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

            let window = app
                .get_webview_window("main")
                .expect("main window should exist");

            let _ = window.set_shadow(false);

            let alt_f = Shortcut::new(Some(Modifiers::ALT), Code::KeyF);
            app.global_shortcut().register(alt_f)?;

            let mut config = config::load_or_create_config(app.handle())
                .map_err(|err| std::io::Error::other(err.to_string()))?;
            let launch_on_startup =
                is_launch_on_startup_enabled(app.handle(), config.launch_on_startup);
            if config.launch_on_startup != launch_on_startup {
                config.launch_on_startup = launch_on_startup;
                config::save_config(app.handle(), &config)
                    .map_err(|err| std::io::Error::other(err.to_string()))?;
            }
            create_tray(app, launch_on_startup)?;

            let db_path = config::database_file_path(app.handle())
                .map_err(|err| std::io::Error::other(err.to_string()))?;
            db::initialize_database(&db_path)
                .map_err(|err| std::io::Error::other(err.to_string()))?;
            let connection = db::open_connection(&db_path)
                .map_err(|err| std::io::Error::other(err.to_string()))?;

            let state = AppState {
                config: Arc::new(Mutex::new(config)),
                db: Arc::new(Mutex::new(connection)),
                watcher: Mutex::new(None),
            };

            {
                let workspaces = {
                    let guard = state.config.lock().expect("config lock poisoned");
                    guard.workspaces.clone()
                };
                let exclude_patterns = {
                    let guard = state.config.lock().expect("config lock poisoned");
                    guard.exclude_patterns.clone()
                };
                let mut conn = state.db.lock().expect("db lock poisoned");
                for workspace in &workspaces {
                    if let Err(err) =
                        indexer::rebuild_workspace(&mut conn, workspace, &exclude_patterns)
                    {
                        log::warn!("初始化索引失败: {err}");
                    }
                }
            }

            let watcher =
                watcher::WatcherService::start(Arc::clone(&state.config), Arc::clone(&state.db))
                    .map_err(|err| std::io::Error::other(err.to_string()))?;
            state
                .watcher
                .lock()
                .expect("watcher lock poisoned")
                .replace(watcher);

            app.manage(state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_workspaces,
            add_workspace,
            search,
            read_preview
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            RunEvent::Ready => hide_main_window(app),
            RunEvent::WindowEvent { label, event, .. } => {
                if label != "main" {
                    return;
                }

                match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        hide_main_window(app);
                    }
                    WindowEvent::Focused(false) => hide_main_window(app),
                    _ => {}
                }
            }
            _ => {}
        });
}
