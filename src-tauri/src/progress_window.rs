//! 任务进度小窗（票 08）：每任务独立窗口，参照 FluxDown 规则——
//! 仅单个任务手动启动时弹出；批量不弹；主窗口关闭与否不影响下载。

use serde_json::{json, Value};
use tauri::{Emitter, Manager};

use crate::settings;

fn window_label(task_id: &str) -> String {
    format!("progress_{task_id}")
}

/// 为任务打开（或聚焦）进度小窗。mode: "progress" | "completion"
pub fn open_for_task(app: &tauri::AppHandle, task_id: &str, mode: &str) -> Result<(), String> {
    let label = window_label(task_id);
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.set_focus();
        let _ = existing.emit("progress://view", json!({ "taskId": task_id, "view": mode }));
        return Ok(());
    }

    tauri::WebviewWindowBuilder::new(
        app,
        &label,
        tauri::WebviewUrl::App(format!("index.html#/progress/{task_id}").into()),
    )
    .title("下载进度")
    .inner_size(360.0, 148.0)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .decorations(false)
    .shadow(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .center()
    .build()
    .map_err(|e| format!("打开进度小窗失败: {e}"))?;

    // 初始视图模式
    let _ = app.emit("progress://view", json!({ "taskId": task_id, "view": mode }));
    Ok(())
}

pub fn close_for_task(app: &tauri::AppHandle, task_id: &str) {
    if let Some(window) = app.get_webview_window(&window_label(task_id)) {
        let _ = window.close();
    }
}

/// 通知某任务的窗口切换到完成视图；按全局设置决定是否自动关闭小窗
pub fn show_completion(app: &tauri::AppHandle, task_id: &str) {
    let _ = app.emit(
        "progress://view",
        json!({ "taskId": task_id, "view": "completion" }),
    );
    if !settings::get().progress_window_on_completion {
        close_for_task(app, task_id);
    }
}

#[tauri::command]
pub async fn show_progress_window(
    app: tauri::AppHandle,
    task_id: String,
    mode: Option<String>,
) -> Result<Value, String> {
    open_for_task(&app, &task_id, mode.as_deref().unwrap_or("progress"))?;
    Ok(json!({ "ok": true }))
}

#[tauri::command]
pub async fn close_progress_window(app: tauri::AppHandle, task_id: String) -> Result<Value, String> {
    close_for_task(&app, &task_id);
    Ok(json!({ "ok": true }))
}
