//! 夸克 API 代理与账号校验。请求头由调用方通过 [`CookieJar`] 提供。

use std::collections::HashMap;

use bytes::Bytes;
use reqwest::{Client, Method, redirect};
use serde_json::Value;

use crate::cookie::{account_nickname, account_state_is_logged_in, CookieJar};

pub const HOST_PAN: &str = "pan.quark.cn";
pub const HOST_DRIVE_PC: &str = "drive-pc.quark.cn";
pub const HOST_DRIVE: &str = "drive.quark.cn";
pub const UOP_HOST: &str = "uop.quark.cn";
pub const COMMON_PARAMS: &str = "pr=ucpro&fr=pc&uc_param_str=";
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) quark-cloud-drive/2.5.20 Chrome/100.0.4896.160 Electron/18.3.5.4-b478491100 Safari/537.36 Channel/pckk_other_ch";
pub const LOGOUT_CALLBACK_URL: &str = "https://pan.quark.cn";

/// API 路由映射（上游 quark_client.rs 忠实移植）
#[derive(Debug, Clone)]
pub struct ApiRoute {
    pub path: &'static str,
    pub method: Method,
    pub host: &'static str,
}

pub fn get_api_routes() -> HashMap<&'static str, ApiRoute> {
    let mut m = HashMap::new();
    m.insert("/api/share/token", ApiRoute { path: "/1/clouddrive/share/sharepage/token", method: Method::POST, host: HOST_PAN });
    m.insert("/api/share/detail", ApiRoute { path: "/1/clouddrive/share/sharepage/detail", method: Method::GET, host: HOST_PAN });
    m.insert("/api/share/save", ApiRoute { path: "/1/clouddrive/share/sharepage/save", method: Method::POST, host: HOST_DRIVE_PC });
    m.insert("/api/task", ApiRoute { path: "/1/clouddrive/task", method: Method::GET, host: HOST_DRIVE_PC });
    m.insert("/api/file/download", ApiRoute { path: "/1/clouddrive/file/download", method: Method::POST, host: HOST_DRIVE });
    m.insert("/api/file/delete", ApiRoute { path: "/1/clouddrive/file/delete", method: Method::POST, host: HOST_DRIVE });
    m.insert("/api/member", ApiRoute { path: "/1/clouddrive/member", method: Method::GET, host: HOST_DRIVE });
    m
}

/// 给请求补上超时——上游 quark_client 的 Client 无任何超时（缺陷 5.2），此处修正。
fn client_with_timeout() -> Client {
    Client::builder()
        .redirect(redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("failed to build HTTP client")
}

pub struct ProxyResponse {
    pub status: u16,
    pub headers: reqwest::header::HeaderMap,
    pub body: Bytes,
}

/// 通用代理请求。cookie 的键值已由 CookieJar 归一化（同名覆盖）。
pub async fn proxy_request(
    target_path: &str,
    method: Method,
    cookie: Option<&CookieJar>,
    body: Option<Bytes>,
    query: &str,
    hostname: &str,
    append_common: bool,
) -> Result<ProxyResponse, String> {
    let client = client_with_timeout();

    let mut full_path = target_path.to_string();
    if append_common {
        let sep = if full_path.contains('?') { '&' } else { '?' };
        full_path = format!("{}{}{}", full_path, sep, COMMON_PARAMS);
    }
    if !query.is_empty() {
        let sep = if full_path.contains('?') { '&' } else { '?' };
        full_path = format!("{}{}{}", full_path, sep, query);
    }

    let url = format!("https://{}{}", hostname, full_path);

    let mut req = client
        .request(method, &url)
        .header("User-Agent", USER_AGENT)
        .header("Referer", "https://pan.quark.cn/")
        .header("Origin", "https://pan.quark.cn")
        .header("Accept", "application/json, text/plain, */*")
        .header("Accept-Language", "zh-CN,zh;q=0.9")
        .header("Content-Type", "application/json;charset=UTF-8");

    if let Some(jar) = cookie {
        let header = jar.header();
        if !header.is_empty() {
            req = req.header("Cookie", header);
        }
    }

    if let Some(b) = body {
        req = req.body(b);
    }

    let resp = req.send().await.map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    let headers = resp.headers().clone();
    let body = resp.bytes().await.map_err(|e| e.to_string())?;

    Ok(ProxyResponse { status, headers, body })
}

pub struct CookieResponse {
    pub status: u16,
    pub body: Bytes,
    pub jar: CookieJar,
}

/// 带重定向跟踪的请求；沿途收集 Set-Cookie 并按同名覆盖合并进 jar。
pub async fn fetch_with_cookies(start_url: &str, jar: CookieJar) -> Result<CookieResponse, String> {
    let client = client_with_timeout();
    let mut jar = jar;
    let mut current_url = start_url.to_string();
    let mut redirects = 0u32;

    loop {
        if redirects > 5 {
            return Err("Too many redirects".into());
        }

        let resp = client
            .get(&current_url)
            .header("User-Agent", USER_AGENT)
            .header("Referer", "https://pan.quark.cn/")
            .header("Accept", "text/html,application/json,*/*")
            .header("Cookie", jar.header())
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let set_cookies: Vec<String> = resp
            .headers()
            .get_all("set-cookie")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .collect();
        jar.merge_response(&set_cookies);

        let status = resp.status().as_u16();

        if (300..400).contains(&status) {
            if let Some(loc) = resp.headers().get("location") {
                let loc_str = loc.to_str().map_err(|e| e.to_string())?;
                current_url = url::Url::parse(&current_url)
                    .and_then(|base| base.join(loc_str))
                    .map(|u| u.to_string())
                    .unwrap_or_else(|_| loc_str.to_string());
                redirects += 1;
                continue;
            }
        }

        let body = resp.bytes().await.map_err(|e| e.to_string())?;
        return Ok(CookieResponse { status, body, jar });
    }
}

pub struct AccountVerifyResult {
    pub status_code: u16,
    pub is_logged_in: bool,
    pub nickname: String,
    pub message: String,
    pub code: String,
    pub payload: Value,
}

/// 用 cookie 检查账号状态（/account/info）
pub async fn fetch_account_info_by_cookie(cookie: &CookieJar) -> Result<AccountVerifyResult, String> {
    let result = proxy_request(
        "/account/info",
        Method::GET,
        Some(cookie),
        None,
        "fr=pc&platform=pc",
        HOST_PAN,
        false,
    )
    .await?;

    let payload: Value = serde_json::from_slice(&result.body).unwrap_or(Value::Null);
    Ok(AccountVerifyResult {
        status_code: result.status,
        is_logged_in: account_state_is_logged_in(result.status, &payload),
        nickname: account_nickname(&payload),
        message: payload
            .get("message")
            .or_else(|| payload.get("msg"))
            .or_else(|| payload.get("error"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        code: payload
            .get("code")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_cover_expected_endpoints() {
        let routes = get_api_routes();
        assert_eq!(routes.len(), 7);
        assert!(routes.contains_key("/api/share/save"));
    }
}
