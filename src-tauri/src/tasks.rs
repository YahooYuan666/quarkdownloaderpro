//! 任务中心：队列、状态机、转存编排、进度事件（票 05）。
//!
//! 「转存 → 轮询 → 取直链 → 删转存 → 下载」全流程在 Rust 侧完成，
//! 前端只订阅 `task-event` 渲染。轮询上限 120s（上游 10s 硬编码导致大包误判超时）。
//! 取消是任务级的（CancellationToken），页面刷新不再影响下载。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::Emitter;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use download_engine::{DownloadOptions, ProgressEvent};
use quark_core::CookieJar;

use crate::cookie_store;

const SAVE_POLL_TIMEOUT_SECS: u64 = 120;
const SAVE_POLL_INTERVAL_MS: u64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Saving,
    Downloading,
    Merging,
    Done,
    Failed,
    Cancelled,
    Skipped,
}

impl TaskStatus {
    /// 终结态：任务不会再有任何后续变更（批量结算据此判断）
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            TaskStatus::Done | TaskStatus::Failed | TaskStatus::Cancelled | TaskStatus::Skipped
        )
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub filename: String,
    pub size: u64,
    pub status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub downloaded: u64,
    pub speed: f64,
    /// 完成后的本地保存路径（用于「打开所在文件夹」，票 06）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saved_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInput {
    pub fid: String,
    pub share_fid_token: String,
    pub file_name: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
struct TaskRuntime {
    task: Task,
    ctx: TaskCtx,
    cancel: CancellationToken,
}

#[derive(Debug, Clone)]
struct TaskCtx {
    pwd_id: String,
    stoken: String,
    fid: String,
    share_fid_token: String,
    file_name: String,
    threads: usize,
    strategy: String,
}

struct TaskState {
    order: Vec<String>,
    runtime: HashMap<String, TaskRuntime>,
}

static TASKS: Mutex<Option<TaskState>> = Mutex::new(None);

fn with_state<T>(f: impl FnOnce(&mut TaskState) -> T) -> T {
    let mut guard = TASKS.lock().unwrap();
    let state = guard.get_or_insert_with(|| TaskState {
        order: Vec::new(),
        runtime: HashMap::new(),
    });
    f(state)
}

fn emit_task(app: &tauri::AppHandle, task: &Task) {
    let _ = app.emit("task-event", json!({ "type": "upsert", "task": task }));
}

fn emit_batch_done(app: &tauri::AppHandle) {
    // 主窗口已隐藏（用户关窗但下载未完成）→ 任务全部终结后退出应用
    if let Some(main) = {
        use tauri::Manager;
        app.get_webview_window("main")
    } {
        if !main.is_visible().unwrap_or(true) {
            app.exit(0);
        }
    }
    let summary = with_state(|s| {
        let mut done = 0usize;
        let mut failed = 0usize;
        let mut cancelled = 0usize;
        for id in &s.order {
            match s.runtime[id].task.status {
                TaskStatus::Done => done += 1,
                TaskStatus::Failed => failed += 1,
                TaskStatus::Cancelled => cancelled += 1,
                _ => {}
            }
        }
        json!({ "done": done, "failed": failed, "cancelled": cancelled, "total": s.order.len() })
    });
    let _ = app.emit("task-event", json!({ "type": "batch-done", "summary": summary }));
}

fn set_status(app: &tauri::AppHandle, id: &str, status: TaskStatus, error: Option<String>) {
    let task = with_state(|s| {
        if let Some(rt) = s.runtime.get_mut(id) {
            rt.task.status = status;
            rt.task.error = error;
            Some(rt.task.clone())
        } else {
            None
        }
    });
    if let Some(t) = task {
        emit_task(app, &t);
    }
}

/// 启动一批下载任务。每个任务立刻以排队态登记，由信号量按并发数调度。
#[tauri::command]
pub async fn start_batch(
    app: tauri::AppHandle,
    files: Vec<TaskInput>,
    pwd_id: String,
    stoken: String,
    concurrency: usize,
    threads: usize,
    strategy_override: Option<String>,
) -> Result<Value, String> {
    if files.is_empty() {
        return Err("没有可下载的文件".into());
    }
    if cookie_store::header().is_empty() {
        return Err("未登录：请先在应用内登录夸克账号".into());
    }

    tracing::info!(count = files.len(), pwd_id = %pwd_id, concurrency, threads, "收到批量下载请求");

    let strategy_override = strategy_override.filter(|s| matches!(s.as_str(), "rename" | "skip"));
    let ids: Vec<String> = files
        .iter()
        .map(|_| uuid::Uuid::new_v4().to_string()[..8].to_string())
        .collect();

    with_state(|s| {
        for (id, f) in ids.iter().zip(files.iter()) {
            s.order.push(id.clone());
            s.runtime.insert(
                id.clone(),
                TaskRuntime {
                    task: Task {
                        id: id.clone(),
                        filename: f.file_name.clone(),
                        size: f.size,
                        status: TaskStatus::Queued,
                        error: None,
                        downloaded: 0,
                        speed: 0.0,
                        saved_path: None,
                    },
                    ctx: TaskCtx {
                        pwd_id: pwd_id.clone(),
                        stoken: stoken.clone(),
                        fid: f.fid.clone(),
                        share_fid_token: f.share_fid_token.clone(),
                        file_name: f.file_name.clone(),
                        threads: threads.clamp(1, download_engine::MAX_SEGMENT_COUNT),
                        strategy: strategy_override.clone().unwrap_or_else(|| crate::settings::get().duplicate_strategy),
                    },
                    cancel: CancellationToken::new(),
                },
            );
        }
    });

    // 并发调度：每个任务一个协程，先到先占信号量许可
    let concurrency = concurrency.clamp(1, 10);
    let semaphore = Arc::new(Semaphore::new(concurrency));
    for id in &ids {
        let app = app.clone();
        let id = id.clone();
        let permit_sem = Arc::clone(&semaphore);
        tokio::spawn(async move {
            let permit = permit_sem.acquire().await.expect("信号量已关闭");
            let cancelled = with_state(|s| s.runtime.get(&id).map(|rt| rt.cancel.is_cancelled()).unwrap_or(true));
            if !cancelled {
                run_task(&app, &id).await;
            }
            drop(permit);
            // 全部结算后发汇总
            let settled = with_state(|s| s.order.iter().all(|id| s.runtime[id].task.status.is_terminal()));
            if settled {
                emit_batch_done(&app);
            }
        });
    }

    // 通知前端全部初始任务
    let snapshot = tasks_snapshot().await?;
    let _ = app.emit("task-event", json!({ "type": "snapshot", "tasks": snapshot }));

    // 票 08 触发规则：仅单个任务才弹进度小窗；批量不弹
    if ids.len() == 1 {
        crate::progress_window::open_for_task(&app, &ids[0], "progress")?;
    }
    Ok(json!({ "started": ids.len() }))
}

/// 单个任务的完整编排
async fn run_task(app: &tauri::AppHandle, id: &str) {
    // 快照任务上下文与取消令牌
    let (ctx, cancel) = with_state(|s| {
        let rt = s.runtime.get(id).expect("runtime exists");
        (rt.ctx.clone(), rt.cancel.clone())
    });

    tracing::info!(id = %id, file = %ctx.file_name, "任务开始运行");

    // 已取消则直接结算
    if cancel.is_cancelled() {
        set_status(app, id, TaskStatus::Cancelled, None);
        return;
    }

    // 先进入「转存中」状态，等网盘转存完成、取得直链、引擎开跑时再切换为「下载中」
    set_status(app, id, TaskStatus::Saving, None);

    // 票 06：同名文件策略（rename=追加序号 / skip=跳过；ask 已由前端解析为二者之一）
    let dir = crate::settings::effective_dir();
    let raw_path = dir.join(&ctx.file_name);
    let save_path = if raw_path.exists() && ctx.strategy == "skip" {
        set_status(app, id, TaskStatus::Skipped, Some("同名文件已存在，已跳过".into()));
        return;
    } else if raw_path.exists() {
        crate::resolve_save_path(&dir, &ctx.file_name)
    } else {
        raw_path
    };

    let result = orchestrate(app, id, &ctx, &cancel, save_path).await;

    match result {
        Ok(saved_path) => {
            tracing::info!(id = %id, path = %saved_path, "任务已顺利完成");
            with_state(|s| {
                if let Some(rt) = s.runtime.get_mut(id) {
                    rt.task.status = TaskStatus::Done;
                    rt.task.downloaded = rt.task.size;
                    rt.task.speed = 0.0;
                    rt.task.saved_path = Some(saved_path);
                }
            });
            let t = with_state(|s| s.runtime[id].task.clone());
            emit_task(app, &t);
            crate::progress_window::show_completion(app, id);
        }
        Err(e) => {
            tracing::error!(id = %id, error = %e, "任务失败或取消");
            let status = if cancel.is_cancelled() { TaskStatus::Cancelled } else { TaskStatus::Failed };
            set_status(app, id, status, Some(e));
            if cancel.is_cancelled() {
                crate::progress_window::close_for_task(app, id);
            }
        }
    }
}

async fn orchestrate(
    app: &tauri::AppHandle,
    id: &str,
    ctx: &TaskCtx,
    cancel: &CancellationToken,
    save_path: std::path::PathBuf,
) -> Result<String, String> {
    let jar = jar_or_err()?;
    let routes = quark_core::protocol::get_api_routes();

    // 1. 转存到网盘
    let save_body = serde_json::to_vec(&json!({
        "fid_list": [ctx.fid],
        "fid_token_list": [ctx.share_fid_token],
        "to_pdir_fid": "0",
        "pwd_id": ctx.pwd_id,
        "stoken": ctx.stoken,
        "pdir_fid": "0",
        "scene": "link",
    }))
    .unwrap();
    let save_resp = quark_core::protocol::proxy_request(
        routes.get("/api/share/save").unwrap().path,
        routes.get("/api/share/save").unwrap().method.clone(),
        Some(&jar),
        Some(save_body.into()),
        "",
        routes.get("/api/share/save").unwrap().host,
        true,
    )
    .await
    .map_err(|e| format!("转存请求失败: {e}"))?;
    cookie_store::update_from_set_cookie(&cookie_store::collect_set_cookies(&save_resp.headers));
    let save_json: Value = serde_json::from_slice(&save_resp.body).unwrap_or(Value::Null);
    if save_resp.status != 200 || save_json.get("data").is_none() {
        return Err(format!("转存失败: {}", upstream_message(&save_json)));
    }
    let save_task_id = save_json["data"]["task_id"]
        .as_str()
        .ok_or("转存失败: 未返回 task_id")?
        .to_string();

    // 2. 轮询转存任务（上限 120s，上游 10s 是大包误报超时的根因）
    let mut saved_fid: Option<String> = None;
    for r in 0..SAVE_POLL_TIMEOUT_SECS {
        if cancel.is_cancelled() {
            return Err("下载已取消".into());
        }
        tokio::time::sleep(Duration::from_millis(SAVE_POLL_INTERVAL_MS)).await;
        let query = format!("task_id={}&retry_index={}", urlencoding::encode(&save_task_id), r);
        let poll = quark_core::protocol::proxy_request(
            routes.get("/api/task").unwrap().path,
            routes.get("/api/task").unwrap().method.clone(),
            Some(&jar),
            None,
            &query,
            routes.get("/api/task").unwrap().host,
            true,
        )
        .await
        .map_err(|e| format!("转存轮询失败: {e}"))?;
        cookie_store::update_from_set_cookie(&cookie_store::collect_set_cookies(&poll.headers));
        let poll_json: Value = serde_json::from_slice(&poll.body).unwrap_or(Value::Null);

        // 检查任务状态：2=成功；0/1=排队/处理中；其它=失败（例如空间不足被夸克拒绝）
        let status = poll_json["data"]["status"].as_i64();
        if status == Some(2) {
            saved_fid = poll_json["data"]["save_as"]["save_as_top_fids"][0]
                .as_str()
                .map(|s| s.to_string());
            break;
        } else if let Some(s) = status {
            if s != 0 && s != 1 {
                let msg = upstream_message(&poll_json);
                let reason = if msg != "未知错误" && !msg.is_empty() {
                    msg
                } else if let Some(m) = poll_json["data"]["message"].as_str() {
                    m.to_string()
                } else {
                    format!("网盘转存失败 (status={s})，多为夸克网盘剩余空间不足，请先清理网盘")
                };
                return Err(format!("转存失败: {reason}"));
            }
        } else if poll_json.get("code").and_then(|c| c.as_i64()) != Some(0) && poll_json.get("status").and_then(|s| s.as_i64()) != Some(200) {
            let msg = upstream_message(&poll_json);
            return Err(format!("转存失败: {msg}"));
        }
    }
    let saved_fid = saved_fid.ok_or_else(|| "转存超时：网盘处理超时，可能当前转存大文件较多或容量不足".to_string())?;
    crate::journal::record(&saved_fid);

    if cancel.is_cancelled() {
        return Err("下载已取消".into());
    }

    // 3. 获取直链
    let dl_body = serde_json::to_vec(&json!({ "fids": [saved_fid] })).unwrap();
    let dl_resp = quark_core::protocol::proxy_request(
        routes.get("/api/file/download").unwrap().path,
        routes.get("/api/file/download").unwrap().method.clone(),
        Some(&jar),
        Some(dl_body.into()),
        "",
        routes.get("/api/file/download").unwrap().host,
        true,
    )
    .await
    .map_err(|e| format!("获取直链失败: {e}"))?;
    cookie_store::update_from_set_cookie(&cookie_store::collect_set_cookies(&dl_resp.headers));
    let dl_json: Value = serde_json::from_slice(&dl_resp.body).unwrap_or(Value::Null);
    if quark_core::is_auth_failure(&dl_json) {
        crate::notify_session_expired("登录状态已失效".into());
    }
    if dl_resp.status != 200 {
        return Err(format!("获取直链失败: {}", upstream_message(&dl_json)));
    }
    let download_url = dl_json["data"][0]["download_url"]
        .as_str()
        .ok_or_else(|| format!("获取直链失败: {}", upstream_message(&dl_json)))?
        .to_string();

    // 4. 删除转存文件（直链自带鉴权 token，删除后仍有效——上游注释）
    let del_body = serde_json::to_vec(&json!({
        "action_type": 2,
        "filelist": [saved_fid],
        "exclude_fids": [],
    }))
    .unwrap();
    let del = quark_core::protocol::proxy_request(
        routes.get("/api/file/delete").unwrap().path,
        routes.get("/api/file/delete").unwrap().method.clone(),
        Some(&jar),
        Some(del_body.into()),
        "",
        routes.get("/api/file/delete").unwrap().host,
        true,
    )
    .await;
    if let Ok(resp) = del {
        cookie_store::update_from_set_cookie(&cookie_store::collect_set_cookies(&resp.headers));
        if resp.status == 200 {
            crate::journal::remove(&saved_fid);
        }
    }

    // 5. 引擎下载（此时直链已就位，正式切换为「下载中」）
    set_status(app, id, TaskStatus::Downloading, None);

    let options = DownloadOptions {
        url: download_url,
        headers: vec![
            ("User-Agent".into(), crate::USER_AGENT.into()),
            ("Cookie".into(), cookie_store::header()),
            ("Referer".into(), "https://pan.quark.cn/".into()),
            ("Accept".into(), "*/*".into()),
            ("Accept-Encoding".into(), "identity".into()),
        ],
        save_path: save_path.clone(),
        segment_count: ctx.threads,
    };

    let emit_app = app.clone();
    let task_id = id.to_string();
    let total_size = with_state(|s| s.runtime[id].task.size);

    let result = download_engine::download(options, cancel.clone(), move |ev| match ev {
        ProgressEvent::Started { total } => {
            with_state(|s| {
                if let Some(rt) = s.runtime.get_mut(&task_id) {
                    rt.task.size = total;
                }
            });
        }
        ProgressEvent::Progress { downloaded, total, speed } => {
            let task = with_state(|s| {
                if let Some(rt) = s.runtime.get_mut(&task_id) {
                    rt.task.downloaded = downloaded;
                    rt.task.size = if total > 0 { total } else { rt.task.size };
                    rt.task.speed = speed;
                    Some(rt.task.clone())
                } else {
                    None
                }
            });
            if let Some(t) = task {
                let _ = emit_app.emit("task-event", json!({ "type": "progress", "task": t }));
            }
        }
        ProgressEvent::Merging => {
            set_status(&emit_app, &task_id, TaskStatus::Merging, None);
        }
        ProgressEvent::Done { .. } => {
            let _ = total_size;
        }
    })
    .await;

    match result {
        Ok(_) => Ok(save_path.to_string_lossy().to_string()),
        Err(download_engine::DownloadError::Cancelled) => Err("下载已取消".into()),
        Err(e) => Err(e.to_string()),
    }
}

fn jar_or_err() -> Result<CookieJar, String> {
    let jar = cookie_store::snapshot().unwrap_or_default();
    if jar.is_empty() {
        return Err("未登录：请先在应用内登录夸克账号".into());
    }
    Ok(jar)
}

fn upstream_message(v: &Value) -> String {
    v.get("message")
        .or_else(|| v.get("msg"))
        .and_then(|m| m.as_str())
        .unwrap_or("未知错误")
        .to_string()
}

/// 是否仍有未终结的任务（主窗口关闭拦截用）
pub fn has_active_tasks() -> bool {
    with_state(|s| s.runtime.values().any(|rt| !rt.task.status.is_terminal()))
}

/// 活跃任务数量（关窗确认用）
pub fn active_task_count() -> usize {
    with_state(|s| s.runtime.values().filter(|rt| !rt.task.status.is_terminal()).count())
}

/// 同步取消全部任务（退出前调用，不再等待收尾）
pub fn cancel_all_now() {
    with_state(|s| {
        for rt in s.runtime.values() {
            rt.cancel.cancel();
        }
    });
}

/// 取消单个任务
#[tauri::command]
pub async fn task_cancel(app: tauri::AppHandle, id: String) -> Result<Value, String> {
    // 先取消令牌：已进入 run_task 的任务由它自己收尾为 Cancelled
    let token = with_state(|s| s.runtime.get(&id).map(|rt| rt.cancel.clone()));
    if let Some(t) = token {
        t.cancel();
    }
    // 尚未开跑（排队中）的任务不会被 run_task 接手，这里立即结算，
    // 否则该行会永远停在「排队中」。
    let queued = with_state(|s| {
        if let Some(rt) = s.runtime.get_mut(&id) {
            if rt.task.status == TaskStatus::Queued {
                rt.task.status = TaskStatus::Cancelled;
                rt.task.error = Some("已取消".into());
                return Some(rt.task.clone());
            }
        }
        None
    });
    if let Some(t) = queued {
        emit_task(&app, &t);
        let settled = with_state(|s| s.order.iter().all(|id| s.runtime[id].task.status.is_terminal()));
        if settled {
            emit_batch_done(&app);
        }
    }
    Ok(json!({ "ok": true }))
}

/// 取消全部活跃任务
#[tauri::command]
pub async fn task_cancel_all(app: tauri::AppHandle) -> Result<Value, String> {
    with_state(|s| {
        for rt in s.runtime.values() {
            if matches!(rt.task.status, TaskStatus::Queued | TaskStatus::Saving | TaskStatus::Downloading | TaskStatus::Merging) {
                rt.cancel.cancel();
            }
        }
    });
    // 取消即结算：状态机由 run_task 收尾，但排队中的任务不会进入 run_task，
    // 这里直接把排队任务标记为已取消。
    let cancelled: Vec<Task> = with_state(|s| {
        let mut out = Vec::new();
        for rt in s.runtime.values_mut() {
            if rt.task.status == TaskStatus::Queued {
                rt.task.status = TaskStatus::Cancelled;
                rt.task.error = Some("已取消".into());
                out.push(rt.task.clone());
            }
        }
        out
    });
    for t in cancelled {
        emit_task(&app, &t);
    }
    // 不在此处发 batch-done：仍在跑的活跃任务需要时间落到 Cancelled，
    // 由调度协程在全部终结后统一结算（提前发会让界面误报"全部完成"）。
    Ok(json!({ "ok": true }))
}

/// 重试失败任务
#[tauri::command]
pub async fn task_retry(app: tauri::AppHandle, id: String) -> Result<Value, String> {
    let ok = with_state(|s| {
        if let Some(rt) = s.runtime.get_mut(&id) {
            if rt.task.status == TaskStatus::Failed {
                rt.task.status = TaskStatus::Queued;
                rt.task.error = None;
                rt.task.downloaded = 0;
                rt.task.speed = 0.0;
                rt.cancel = CancellationToken::new();
                return true;
            }
        }
        false
    });
    if !ok {
        return Err("只有失败的任务可以重试".into());
    }
    let t = with_state(|s| s.runtime[&id].task.clone());
    emit_task(&app, &t);

    // 重新调度（重试为用户显式行为，单独占一个协程即可）
    let app2 = app.clone();
    let id2 = id.clone();
    tokio::spawn(async move {
        run_task(&app2, &id2).await;
        let settled = with_state(|s| s.order.iter().all(|id| s.runtime[id].task.status.is_terminal()));
        if settled {
            emit_batch_done(&app2);
        }
    });
    Ok(json!({ "ok": true }))
}

/// 当前全部任务快照
#[tauri::command]
pub async fn tasks_snapshot() -> Result<Vec<Task>, String> {
    Ok(with_state(|s| s.order.iter().map(|id| s.runtime[id].task.clone()).collect()))
}
