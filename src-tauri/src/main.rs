#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod power;

use playstreamer_core::{
    AppConfig, DiagnosticReport, Engine, NetworkInterface, PLAYBACK_URL, Snapshot, platform,
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{
    Emitter, Manager, State,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;

struct Desktop {
    engine: Arc<Engine>,
    quit_pending: Arc<AtomicBool>,
    exit_authorized: Arc<AtomicBool>,
    exports: PathBuf,
}
type CommandResult<T> = Result<T, String>;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
async fn get_config(state: State<'_, Desktop>) -> CommandResult<AppConfig> {
    Ok(state.engine.config().await)
}
#[tauri::command]
async fn save_config(config: AppConfig, state: State<'_, Desktop>) -> CommandResult<()> {
    state.engine.save_config(config).await.map_err(err)
}
#[tauri::command]
async fn list_network_interfaces() -> CommandResult<Vec<NetworkInterface>> {
    playstreamer_core::service::interfaces().await.map_err(err)
}
#[tauri::command]
fn get_snapshot(state: State<'_, Desktop>) -> Snapshot {
    state.engine.snapshot()
}
#[tauri::command]
async fn start_receiving(state: State<'_, Desktop>) -> CommandResult<Snapshot> {
    state.engine.start_receiving().await.map_err(err)
}
#[tauri::command]
async fn stop_receiving(state: State<'_, Desktop>) -> CommandResult<Snapshot> {
    state.engine.stop_receiving().await.map_err(err)
}
#[tauri::command]
async fn run_diagnostics(state: State<'_, Desktop>) -> CommandResult<DiagnosticReport> {
    Ok(state.engine.diagnostics().await)
}
#[tauri::command]
async fn export_diagnostics(state: State<'_, Desktop>) -> CommandResult<String> {
    state
        .engine
        .export_diagnostics(&state.exports)
        .await
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(err)
}
#[tauri::command]
async fn configure_firewall(state: State<'_, Desktop>) -> CommandResult<String> {
    let config = state.engine.config().await;
    let list = playstreamer_core::service::interfaces()
        .await
        .map_err(err)?;
    let (lan, ps5, _) = playstreamer_core::service::validate(&config, &list).map_err(err)?;
    let exe = std::env::current_exe().map_err(err)?;
    let binary = state.engine.binary().to_owned();
    tokio::task::spawn_blocking(move || platform::configure_firewall(&exe, &binary, lan, ps5))
        .await
        .map_err(err)?
        .map_err(err)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateInfo {
    current_version: String,
    latest_version: String,
    release_url: String,
    update_available: bool,
}
#[derive(serde::Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
}
#[tauri::command]
async fn check_updates() -> CommandResult<UpdateInfo> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("PlayStreamer")
        .build()
        .map_err(err)?;
    let release: Release = client
        .get("https://api.github.com/repos/JxQg/PlayStreamer/releases/latest")
        .send()
        .await
        .map_err(err)?
        .error_for_status()
        .map_err(err)?
        .json()
        .await
        .map_err(err)?;
    let latest = release.tag_name.trim_start_matches('v').to_owned();
    let current = env!("CARGO_PKG_VERSION").to_owned();
    Ok(UpdateInfo {
        update_available: version_tuple(&latest) > version_tuple(&current),
        current_version: current,
        latest_version: latest,
        release_url: release.html_url,
    })
}
fn version_tuple(s: &str) -> (u64, u64, u64) {
    let mut p = s.split('.').map(|x| x.parse::<u64>().unwrap_or(0));
    (
        p.next().unwrap_or(0),
        p.next().unwrap_or(0),
        p.next().unwrap_or(0),
    )
}
#[tauri::command]
fn open_external(url: String, app: tauri::AppHandle) -> CommandResult<()> {
    let allowed = [
        "https://obsproject.com",
        "https://link.bilibili.com",
        "https://oopz.cn",
        "https://github.com/JxQg/PlayStreamer",
        "https://www.playstation.com",
        "https://www.twitch.tv",
    ];
    if !allowed
        .iter()
        .any(|base| url == *base || url.starts_with(&format!("{base}/")))
    {
        return Err("此链接不在应用的官方教程列表中".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(err)
}
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    request_quit(app);
}

fn show(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}
fn request_quit(app: tauri::AppHandle) {
    let state = app.state::<Desktop>();
    if state.quit_pending.swap(true, Ordering::SeqCst) {
        return;
    }
    let engine = state.engine.clone();
    let quit_pending = state.quit_pending.clone();
    let exit_authorized = state.exit_authorized.clone();
    let finish_app = app.clone();
    let finish = move |accepted: bool| {
        if accepted {
            tauri::async_runtime::spawn(async move {
                if let Err(e) = engine.shutdown().await {
                    finish_app
                        .dialog()
                        .message(format!("服务回收失败：{e}"))
                        .kind(MessageDialogKind::Error)
                        .show(|_| {});
                    quit_pending.store(false, Ordering::SeqCst);
                    return;
                }
                exit_authorized.store(true, Ordering::SeqCst);
                finish_app.exit(0);
            });
        } else {
            quit_pending.store(false, Ordering::SeqCst);
        }
    };
    if state.engine.snapshot().dns_running {
        app.dialog().message("请先停止 PS5 广播，并把 PS5 的 DNS 改回自动。退出后电脑不再提供 DNS，PS5 可能无法联网。").title("退出 PlayStreamer").buttons(MessageDialogButtons::OkCancelCustom("退出应用".into(),"继续运行".into())).show(finish);
    } else {
        finish(true);
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show(app)))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            let exports = app
                .path()
                .document_dir()
                .unwrap_or_else(|_| data.clone())
                .join("PlayStreamer");
            let binary = if cfg!(debug_assertions) {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("binaries/mediamtx-x86_64-pc-windows-msvc.exe")
            } else {
                std::env::current_exe()?
                    .parent()
                    .ok_or("无法定位安装目录")?
                    .join(if cfg!(windows) {
                        "mediamtx.exe"
                    } else {
                        "mediamtx"
                    })
            };
            let engine = tauri::async_runtime::block_on(Engine::new(
                data.join("config.json"),
                binary,
                data.join("runtime"),
            ))?;
            power::watch(&engine);
            let handle = app.handle().clone();
            let watcher = engine.clone();
            tauri::async_runtime::spawn(async move {
                watcher.start_monitor();
                let mut rx = watcher.subscribe();
                while rx.changed().await.is_ok() {
                    let snapshot = rx.borrow().clone();
                    let _ = handle.emit("service-state", &snapshot);
                    if let Some(tray) = handle.tray_by_id("main") {
                        let _ =
                            tray.set_tooltip(Some(format!("PlayStreamer · {}", snapshot.message)));
                    }
                }
            });
            app.manage(Desktop {
                engine,
                quit_pending: Arc::new(AtomicBool::new(false)),
                exit_authorized: Arc::new(AtomicBool::new(false)),
                exports,
            });
            let show_i = MenuItem::with_id(app, "show", "显示 PlayStreamer", true, None::<&str>)?;
            let start_i = MenuItem::with_id(app, "start", "开启接收", true, None::<&str>)?;
            let stop_i = MenuItem::with_id(app, "stop", "停止接收，保留 DNS", true, None::<&str>)?;
            let copy_i = MenuItem::with_id(app, "copy", "复制播放地址", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &start_i, &stop_i, &copy_i, &quit_i])?;
            TrayIconBuilder::with_id("main")
                .icon(tauri::image::Image::from_bytes(include_bytes!(
                    "../icons/tray.png"
                ))?)
                .tooltip("PlayStreamer")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show(app),
                    "copy" => {
                        if let Err(e) = app.clipboard().write_text(PLAYBACK_URL) {
                            app.dialog()
                                .message(e.to_string())
                                .kind(MessageDialogKind::Error)
                                .show(|_| {});
                        }
                    }
                    "start" | "stop" => {
                        let app = app.clone();
                        let start = event.id.as_ref() == "start";
                        let engine = app.state::<Desktop>().engine.clone();
                        tauri::async_runtime::spawn(async move {
                            let result = if start {
                                engine.start_receiving().await
                            } else {
                                engine.stop_receiving().await
                            };
                            if let Err(e) = result {
                                show(&app);
                                app.dialog()
                                    .message(e.to_string())
                                    .kind(MessageDialogKind::Error)
                                    .show(|_| {});
                            }
                        });
                    }
                    "quit" => request_quit(app.clone()),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show(tray.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            list_network_interfaces,
            get_snapshot,
            start_receiving,
            stop_receiving,
            run_diagnostics,
            export_diagnostics,
            configure_firewall,
            check_updates,
            open_external,
            quit_app
        ])
        .build(tauri::generate_context!())
        .expect("无法初始化 PlayStreamer")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !app
                    .state::<Desktop>()
                    .exit_authorized
                    .load(Ordering::SeqCst)
                {
                    api.prevent_exit();
                    request_quit(app.clone());
                }
            }
        });
}
