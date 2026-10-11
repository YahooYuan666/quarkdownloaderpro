//! 网页登录窗口：打开一个真实的夸克网页，用户用任何方式正常登录，
//! 应用自动把登录 Cookie 读走并存入系统凭据库。
//!
//! 为什么这样做：
//! - 之前的扫码登录是在 Rust 侧用 HTTP 请求模拟浏览器，夸克的二维码接口
//!   对非浏览器环境不稳定，网络稍差就取不到码；
//! - 手动复制 Cookie 又太繁琐。
//! 内嵌 webview 打开官网就是「真浏览器」——扫码/短信/密码登录全部可用，
//! 用户不需要知道 Cookie 是什么。

use std::time::Duration;

use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use quark_core::CookieJar;

const LOGIN_URL: &str = "https://pan.quark.cn";
const WINDOW_LABEL: &str = "quark_login";
const POLL_INTERVAL: Duration = Duration::from_millis(1500);
const TIMEOUT: Duration = Duration::from_secs(300);

/// 打开（或聚焦）网页登录窗口，并在后台轮询检测登录是否完成
pub fn open(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window(WINDOW_LABEL) {
        let _ = existing.set_focus();
        return Ok(());
    }

    let url = LOGIN_URL
        .parse()
        .map_err(|e| format!("登录地址无效: {e}"))?;

    let window = WebviewWindowBuilder::new(app, WINDOW_LABEL, WebviewUrl::External(url))
        .title("登录夸克网盘（登录成功后本窗口会自动关闭）")
        .inner_size(1040.0, 760.0)
        .min_inner_size(640.0, 520.0)
        // 登录期间必须始终可见：否则容易被主窗口盖住，用户以为没反应
        .always_on_top(true)
        .focused(true)
        .center()
        .build()
        .map_err(|e| format!("打开登录窗口失败: {e}"))?;

    // 双保险：创建后再显式置前一次（某些窗口管理器下 focused 不生效）
    let _ = window.unminimize();
    let _ = window.set_focus();

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let started = std::time::Instant::now();
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;

            // 窗口被用户关掉 → 结束轮询
            let Some(win) = handle.get_webview_window(WINDOW_LABEL) else {
                return;
            };

            if started.elapsed() > TIMEOUT {
                let _ = win.close();
                let _ = handle.emit("login://timeout", serde_json::json!({}));
                return;
            }

            // 读取该窗口在夸克域下的 Cookie（与真实浏览器同一份存储）
            let Ok(cookies) = win.cookies() else {
                continue;
            };
            if cookies.is_empty() {
                continue;
            }

            let mut jar = CookieJar::default();
            for c in &cookies {
                let domain = c.domain().unwrap_or("");
                if domain.contains("quark.cn") {
                    jar.set(c.name().to_string(), c.value().to_string());
                }
            }
            // 夸克的关键会话字段；没有它一定还没登录完成
            if !jar.contains_key("__puus") && !jar.contains_key("__pus") {
                continue;
            }

            // 用读到的 Cookie 去校验账号，通过才算真的登录成功
            match quark_core::protocol::fetch_account_info_by_cookie(&jar).await {
                Ok(info) if info.is_logged_in => {
                    let header = jar.header();
                    crate::cookie_store::set_from_text(&header);
                    let _ = handle.emit(
                        "login://done",
                        serde_json::json!({ "nickname": info.nickname }),
                    );
                    if let Some(w) = handle.get_webview_window(WINDOW_LABEL) {
                        let _ = w.close();
                    }
                    return;
                }
                // 尚未登录完成（或页面还在跳转），继续等
                _ => continue,
            }
        }
    });

    Ok(())
}

/// 用户主动取消网页登录
pub fn cancel(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window(WINDOW_LABEL) {
        let _ = w.close();
    }
}

#[tauri::command]
pub async fn open_web_login(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    open(&app)?;
    Ok(serde_json::json!({ "opened": true }))
}

#[tauri::command]
pub async fn cancel_web_login(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    cancel(&app);
    Ok(serde_json::json!({ "cancelled": true }))
}
