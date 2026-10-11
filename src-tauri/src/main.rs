// 在 release 模式下隐藏 Windows 控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod cookie_store;
mod journal;
mod login_window;
mod progress_window;
mod settings;
mod tasks;

use std::path::PathBuf;
use std::sync::Mutex as StdMutex;

/// 复用协议层的 UA 常量（单一来源，避免两处各自漂移）
pub use quark_core::protocol::USER_AGENT;

/// 解决文件名冲突：存在同名文件时追加 (1), (2), ...
pub(crate) fn resolve_save_path(downloads_dir: &PathBuf, filename: &str) -> PathBuf {
    let mut save_path = downloads_dir.join(filename);
    if !save_path.exists() {
        return save_path;
    }
    let stem = save_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("download")
        .to_string();
    let ext = save_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let mut i = 1;
    loop {
        let new_name = if ext.is_empty() {
            format!("{} ({})", stem, i)
        } else {
            format!("{} ({}).{}", stem, i, ext)
        };
        save_path = downloads_dir.join(new_name);
        if !save_path.exists() {
            return save_path;
        }
        i += 1;
    }
}

/// 全局 AppHandle：供协议层在任何命令里发出会话失效事件（票 07）
static APP_HANDLE: StdMutex<Option<tauri::AppHandle>> = StdMutex::new(None);

pub fn notify_session_expired(reason: String) {
    if let Some(handle) = APP_HANDLE.lock().unwrap().as_ref() {
        use tauri::Emitter;
        let _ = handle.emit(
            "session://expired",
            serde_json::json!({ "reason": reason }),
        );
    }
}

// ── 会话命令（Cookie 存系统凭据库，前端不接触明文） ────────

/// 保存并立即校验用户粘贴的 Cookie
#[tauri::command]
async fn cookie_set(cookie: String) -> Result<serde_json::Value, String> {
    let jar = cookie_store::set_from_text(&cookie);
    let info = quark_core::protocol::fetch_account_info_by_cookie(&jar)
        .await
        .map_err(|e| format!("校验请求失败: {e}"))?;
    if info.is_logged_in {
        Ok(serde_json::json!({
            "ok": true,
            "nickname": info.nickname,
            "message": "登录成功",
        }))
    } else {
        // 校验失败：不入库
        cookie_store::clear();
        Ok(serde_json::json!({
            "ok": false,
            "nickname": "",
            "message": if info.message.is_empty() { "Cookie 无效或已过期".to_string() } else { info.message },
        }))
    }
}

/// 本地会话状态（不发网络请求）
#[tauri::command]
async fn cookie_state() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({ "has": cookie_store::has_session() }))
}

/// 当前账号信息（发网络请求校验；用于界面常驻登录态展示，票 07）
#[tauri::command]
async fn cookie_whoami() -> Result<serde_json::Value, String> {
    let jar = cookie_store::snapshot().unwrap_or_default();
    if jar.is_empty() {
        return Ok(serde_json::json!({ "logged_in": false, "nickname": "" }));
    }
    let info = quark_core::protocol::fetch_account_info_by_cookie(&jar)
        .await
        .map_err(|e| format!("校验请求失败: {e}"))?;
    Ok(serde_json::json!({
        "logged_in": info.is_logged_in,
        "nickname": info.nickname,
        "message": info.message,
    }))
}

/// 系统凭据库是否可用（不可用时"记住登录"会失效，界面应提示用户）
#[tauri::command]
async fn cookie_storage_ok() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({ "available": cookie_store::storage_available() }))
}

#[tauri::command]
async fn cookie_clear() -> Result<serde_json::Value, String> {
    cookie_store::clear();
    Ok(serde_json::json!({ "ok": true }))
}

/// 读取本地图片并返回 Data URL（用于系统拖放二维码图片时解码）
#[tauri::command]
async fn read_image_data_url(path: String) -> Result<String, String> {
    let p = std::path::Path::new(&path);
    if !p.is_file() {
        return Err("文件不存在或不是普通文件".into());
    }
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        _ => return Err(format!("不支持的图片格式: .{ext}")),
    };

    let meta = std::fs::metadata(p).map_err(|e| format!("读取元数据失败: {e}"))?;
    if meta.len() > 15 * 1024 * 1024 {
        return Err("图片过大（超过 15MB）".into());
    }

    let bytes = std::fs::read(p).map_err(|e| format!("读取文件内容失败: {e}"))?;
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}

// ── 主入口 ───────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            cookie_set,
            cookie_state,
            cookie_whoami,
            cookie_clear,
            cookie_storage_ok,
            api::qr_token,
            api::qr_query,
            api::qr_cookie,
            api::logout_cmd,
            api::share_token,
            api::share_detail,
            api::share_save,
            api::query_task,
            api::file_download_url,
            api::file_delete,
            api::member_info,
            tasks::start_batch,
            tasks::task_cancel,
            tasks::task_cancel_all,
            tasks::task_retry,
            tasks::tasks_snapshot,
            settings::settings_get,
            settings::settings_set,
            settings::check_duplicates,
            settings::open_folder,
            settings::open_file,
            settings::pick_folder,
            progress_window::show_progress_window,
            progress_window::close_progress_window,
            journal::journal_cleanup,
            login_window::open_web_login,
            login_window::cancel_web_login,
            read_image_data_url
        ])
        .setup(|app| {
            use tauri::Manager;

            // 票 12：日志落盘——绝不记录 Cookie 或完整请求头（仅键名与事件摘要）
            let log_dir = app.path().app_log_dir().ok();
            if let Some(dir) = log_dir {
                let _ = std::fs::create_dir_all(&dir);
                let appender = tracing_appender::rolling::daily(dir, "quark-downloader.log");
                let (writer, guard) = tracing_appender::non_blocking(appender);
                tracing_subscriber::fmt()
                    .with_writer(writer)
                    .with_ansi(false)
                    .with_env_filter(tracing_subscriber::EnvFilter::try_new("info").unwrap_or_default())
                    .init();
                std::mem::forget(guard);
                tracing::info!(version = env!("CARGO_PKG_VERSION"), "应用启动");
            }

            // 从系统凭据库恢复会话（失败降级为内存态）
            cookie_store::init();
            let config_dir = app.path().app_config_dir().ok();
            settings::init(config_dir.clone());
            journal::init(config_dir);
            *APP_HANDLE.lock().unwrap() = Some(app.handle().clone());

            // 票 11：启动时清理上次会话可能残留的网盘转存文件
            tauri::async_runtime::spawn(async move {
                match journal::cleanup_stale().await {
                    Ok(n) if n > 0 => tracing::info!(cleaned = n, "已清理残留转存文件"),
                    Ok(_) => {}
                    Err(e) => tracing::warn!(error = e, "残留转存清理失败"),
                }
            });

            let _main_window = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("Quark Downloader Pro")
            .inner_size(1024.0, 750.0)
            .min_inner_size(1024.0, 700.0)
            .resizable(true)
            .decorations(false)
            .shadow(true)
            .center()
            .build()
            .expect("创建主窗口失败");

            // 票 08 / R3.3：主窗口关闭时若仍有下载在跑，隐藏而非退出——
            // 下载生命周期归 Rust 任务中心，关窗不该掐断它。
            let main_window = app.get_webview_window("main");
            if let Some(win) = main_window {
                let handle_for_close = app.handle().clone();
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        if !tasks::has_active_tasks() {
                            return; // 无进行中任务，正常退出
                        }
                        // 有任务在跑：明确询问，避免「点了关闭，下载悄悄没了」
                        api.prevent_close();
                        let h = handle_for_close.clone();
                        tauri::async_runtime::spawn(async move {
                            use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
                            let n = tasks::active_task_count();
                            let confirmed = h
                                .dialog()
                                .message(format!("还有 {n} 个任务正在下载。

退出软件会中断这些任务，确定要退出吗？"))
                                .title("确认退出")
                                .kind(MessageDialogKind::Warning)
                                .buttons(MessageDialogButtons::OkCancelCustom(
                                    "退出并中断".into(),
                                    "继续下载".into(),
                                ))
                                .blocking_show();
                            if confirmed {
                                tasks::cancel_all_now();
                                h.exit(0);
                            }
                        });
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
