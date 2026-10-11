//! Tauri 命令层：替代原内嵌 HTTP 服务的 API 代理（票 04 移除 axum）。
//! 上游响应体逐字透传，前端类型不变；Cookie 全部来自系统凭据库。

use quark_core::protocol::{get_api_routes, proxy_request, ProxyResponse};
use quark_core::{login, CookieJar};
use serde_json::{json, Value};

use crate::cookie_store;

fn forward(result: Result<ProxyResponse, String>) -> Result<Value, String> {
    let resp = result?;
    cookie_store::update_from_set_cookie(&cookie_store::collect_set_cookies(&resp.headers));
    let body: Value = serde_json::from_slice(&resp.body).unwrap_or(Value::Null);
    if quark_core::is_auth_failure(&body) {
        crate::notify_session_expired("登录状态已失效".into());
    }
    Ok(body)
}

fn jar() -> CookieJar {
    cookie_store::snapshot().unwrap_or_default()
}

fn require_session() -> Result<CookieJar, String> {
    let jar = jar();
    if jar.is_empty() {
        return Err("未登录：请先在应用内登录夸克账号".into());
    }
    Ok(jar)
}

// ── 扫码登录（无需会话） ─────────────────────────────────

#[tauri::command]
pub async fn qr_token() -> Result<Value, String> {
    login::qr_get_token().await
}

#[tauri::command]
pub async fn qr_query(token: String) -> Result<Value, String> {
    let resp = login::qr_query_status(&token).await?;
    Ok(serde_json::from_slice(&resp.body).unwrap_or(Value::Null))
}

#[tauri::command]
pub async fn qr_cookie(service_ticket: String) -> Result<Value, String> {
    let value = login::qr_get_cookie(&service_ticket).await?;
    let cookie = value
        .get("cookie")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if cookie.is_empty() {
        return Err("未获取到 Cookie".into());
    }
    // R6.3：Rust 侧直接入库并校验，明文不回传 webview
    let jar = cookie_store::set_from_text(&cookie);
    let info = quark_core::protocol::fetch_account_info_by_cookie(&jar)
        .await
        .map_err(|e| format!("校验请求失败: {e}"))?;
    if !info.is_logged_in {
        cookie_store::clear();
    }
    Ok(json!({
        "ok": info.is_logged_in,
        "nickname": info.nickname,
        "missing_puus": value.get("missing_puus").cloned().unwrap_or(json!(false)),
    }))
}

#[tauri::command]
pub async fn logout_cmd() -> Result<Value, String> {
    let jar = jar();
    login::logout(&jar).await
}

// ── 业务 API（需要会话） ─────────────────────────────────

#[tauri::command]
pub async fn share_token(pwd_id: String, passcode: String) -> Result<Value, String> {
    let jar = require_session()?;
    let routes = get_api_routes();
    let route = routes.get("/api/share/token").unwrap();
    let body = serde_json::to_vec(&json!({ "pwd_id": pwd_id, "passcode": passcode })).unwrap();
    forward(
        proxy_request(route.path, route.method.clone(), Some(&jar), Some(body.into()), "", route.host, true).await,
    )
}

#[tauri::command]
pub async fn share_detail(
    pwd_id: String,
    stoken: String,
    pdir_fid: String,
    page: Option<i64>,
) -> Result<Value, String> {
    let jar = require_session()?;
    let routes = get_api_routes();
    let route = routes.get("/api/share/detail").unwrap();
    let query = format!(
        "pwd_id={}&stoken={}&pdir_fid={}&force=0&_page={}&_size=50&_fetch_total=1&_fetch_sub_dirs=0&_sort=file_type:asc,file_name:asc",
        urlencoding::encode(&pwd_id),
        urlencoding::encode(&stoken),
        urlencoding::encode(&pdir_fid),
        page.unwrap_or(1),
    );
    forward(
        proxy_request(route.path, route.method.clone(), Some(&jar), None, &query, route.host, true).await,
    )
}

#[tauri::command]
pub async fn share_save(
    pwd_id: String,
    stoken: String,
    fid_list: Vec<String>,
    fid_token_list: Vec<String>,
) -> Result<Value, String> {
    let jar = require_session()?;
    let routes = get_api_routes();
    let route = routes.get("/api/share/save").unwrap();
    let body = serde_json::to_vec(&json!({
        "fid_list": fid_list,
        "fid_token_list": fid_token_list,
        "to_pdir_fid": "0",
        "pwd_id": pwd_id,
        "stoken": stoken,
        "pdir_fid": "0",
        "scene": "link",
    }))
    .unwrap();
    forward(
        proxy_request(route.path, route.method.clone(), Some(&jar), Some(body.into()), "", route.host, true).await,
    )
}

#[tauri::command]
pub async fn query_task(task_id: String, retry_index: Option<i64>) -> Result<Value, String> {
    let jar = require_session()?;
    let routes = get_api_routes();
    let route = routes.get("/api/task").unwrap();
    let query = format!(
        "task_id={}&retry_index={}",
        urlencoding::encode(&task_id),
        retry_index.unwrap_or(0)
    );
    forward(
        proxy_request(route.path, route.method.clone(), Some(&jar), None, &query, route.host, true).await,
    )
}

#[tauri::command]
pub async fn file_download_url(fids: Vec<String>) -> Result<Value, String> {
    let jar = require_session()?;
    let routes = get_api_routes();
    let route = routes.get("/api/file/download").unwrap();
    let body = serde_json::to_vec(&json!({ "fids": fids })).unwrap();
    forward(
        proxy_request(route.path, route.method.clone(), Some(&jar), Some(body.into()), "", route.host, true).await,
    )
}

#[tauri::command]
pub async fn file_delete(fids: Vec<String>) -> Result<Value, String> {
    let jar = require_session()?;
    let routes = get_api_routes();
    let route = routes.get("/api/file/delete").unwrap();
    let body = serde_json::to_vec(&json!({
        "action_type": 2,
        "filelist": fids,
        "exclude_fids": [],
    }))
    .unwrap();
    forward(
        proxy_request(route.path, route.method.clone(), Some(&jar), Some(body.into()), "", route.host, true).await,
    )
}

#[tauri::command]
pub async fn member_info() -> Result<Value, String> {
    let jar = require_session()?;
    let routes = get_api_routes();
    let route = routes.get("/api/member").unwrap();
    let query = "fetch_subscribe=true&_ch=home&fetch_identity=true";
    forward(
        proxy_request(route.path, route.method.clone(), Some(&jar), None, query, route.host, true).await,
    )
}
