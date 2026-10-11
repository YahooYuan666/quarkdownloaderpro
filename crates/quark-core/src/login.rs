//! 扫码登录与退出登录（上游逻辑移植，Cookie 收集改为 CookieJar 语义）。

use serde_json::{json, Value};
use urlencoding::encode;

use crate::cookie::CookieJar;
use crate::protocol::{
    fetch_account_info_by_cookie, fetch_with_cookies, proxy_request, AccountVerifyResult,
    ProxyResponse, UOP_HOST, LOGOUT_CALLBACK_URL,
};

/// 获取扫码登录 token。
///
/// 二维码接口在某些网络环境下会间歇性失败（连接被重置、超时、返回空体），
/// 单次请求失败就让用户重试体验很差，因此这里内置最多 3 次带退避的重试，
/// 并把最终失败原因明确带回去，便于界面显示。
pub async fn qr_get_token() -> Result<Value, String> {
    const ATTEMPTS: u32 = 3;
    let mut last_error = String::from("未知错误");

    for attempt in 1..=ATTEMPTS {
        let request_id = uuid::Uuid::new_v4().to_string();
        let path = format!(
            "/cas/ajax/getTokenForQrcodeLogin?client_id=532&v=1.2&request_id={}",
            request_id
        );

        match proxy_request(&path, reqwest::Method::GET, None, None, "", UOP_HOST, false).await {
            Ok(result) => {
                let status = result.status;
                let mut data: Value =
                    serde_json::from_slice(&result.body).unwrap_or(Value::Null);
                let has_token = data
                    .get("data")
                    .and_then(|d| d.get("members"))
                    .and_then(|m| m.get("token"))
                    .and_then(|t| t.as_str())
                    .map(|t| !t.is_empty())
                    .unwrap_or(false);
                if status == 200 && has_token {
                    data.as_object_mut()
                        .map(|o| o.insert("_request_id".into(), Value::String(request_id)));
                    return Ok(data);
                }
                last_error = if status != 200 {
                    format!("二维码服务返回 {status}")
                } else {
                    "二维码服务未返回 token".to_string()
                };
            }
            Err(e) => {
                last_error = e;
            }
        }

        if attempt < ATTEMPTS {
            // 退避后重试：瞬时网络抖动通常一次重试即可恢复
            tokio::time::sleep(std::time::Duration::from_millis(600 * attempt as u64)).await;
        }
    }

    Err(format!(
        "获取二维码失败（已重试 {ATTEMPTS} 次）：{last_error}。可先改用「手动输入 Cookie」登录，或在浏览器能正常打开夸克网盘后重试。"
    ))
}

/// 轮询扫码状态
pub async fn qr_query_status(token: &str) -> Result<ProxyResponse, String> {
    let poll_id = uuid::Uuid::new_v4().to_string();
    let path = format!(
        "/cas/ajax/getServiceTicketByQrcodeToken?client_id=532&v=1.2&token={}&request_id={}",
        encode(token),
        encode(&poll_id)
    );
    proxy_request(&path, reqwest::Method::GET, None, None, "", UOP_HOST, false).await
}

/// 用 service_ticket 换 cookie（含多步补全 __puus）
pub async fn qr_get_cookie(st: &str) -> Result<Value, String> {
    let url = format!(
        "https://pan.quark.cn/account/info?st={}&lw=scan",
        encode(st)
    );

    let result = fetch_with_cookies(&url, CookieJar::default()).await?;
    let mut jar = result.jar;

    // Step 2-4: 尝试补全 __puus（上游逻辑保留）
    let puus_steps = [
        ("Step2", "https://pan.quark.cn/list"),
        ("Step3", "https://drive-pc.quark.cn/1/clouddrive/file/sort?pr=ucpro&fr=pc&uc_param_str=&pdir_fid=0&_page=1&_size=50&_fetch_total=1&_sort=file_type:asc,updated_at:desc"),
        ("Step4", "https://drive.quark.cn/1/clouddrive/member?pr=ucpro&fr=pc&uc_param_str=&fetch_subscribe=true"),
    ];

    for (_step, step_url) in puus_steps {
        if jar.contains_key("__puus") {
            break;
        }
        match fetch_with_cookies(step_url, jar.clone()).await {
            Ok(r) => {
                // 始终采纳返回的 jar：合并是「同名覆盖」，r.jar 已是包含最新
                // 令牌的超集。旧代码 `if r.jar.len() > jar.len()` 会在键数相同但
                // 令牌已轮换时丢弃新值——正是上游 __puus 失效的同类缺陷。
                jar = r.jar;
            }
            Err(_) => {}
        }
    }

    let has_puus = jar.contains_key("__puus");
    let user_info: Value =
        serde_json::from_slice(&result.body).unwrap_or(Value::Null);

    Ok(json!({
        "cookie": jar.header(),
        "user_info": user_info,
        "missing_puus": !has_puus
    }))
}

/// 退出登录：请求远端注销 → 用远端返回的 Cookie 校验账号是否真的失效
pub async fn logout(cookie: &CookieJar) -> Result<Value, String> {
    if cookie.is_empty() {
        return Err("缺少登录 Cookie，请先登录后再退出".into());
    }

    let logout_url = format!(
        "https://pan.quark.cn/account/logout?callback={}",
        encode(LOGOUT_CALLBACK_URL)
    );

    let mut logout_request_error = String::new();
    let mut verify_jar = cookie.clone();

    match fetch_with_cookies(&logout_url, cookie.clone()).await {
        Ok(flow) => {
            if !flow.jar.is_empty() {
                verify_jar = flow.jar;
            }
        }
        Err(e) => {
            logout_request_error = e;
        }
    }

    let mut verify_error = String::new();
    let account = match fetch_account_info_by_cookie(&verify_jar).await {
        Ok(info) => info,
        Err(e) => {
            verify_error = e;
            AccountVerifyResult {
                status_code: 0,
                is_logged_in: false,
                nickname: String::new(),
                message: String::new(),
                code: String::new(),
                payload: Value::Null,
            }
        }
    };

    let logged_out = if verify_error.is_empty() { !account.is_logged_in } else { false };

    let message = if !logout_request_error.is_empty() && logged_out {
        "退出请求返回异常，但账号状态已失效".to_string()
    } else if !logout_request_error.is_empty() && !logged_out {
        "退出请求失败，夸克账号仍处于登录状态".to_string()
    } else if !logged_out && !account.nickname.is_empty() {
        format!("退出未生效，当前仍为账号「{}」", account.nickname)
    } else if logged_out {
        "夸克账号已退出登录".to_string()
    } else {
        "夸克账号仍处于登录状态，请稍后重试".to_string()
    };

    Ok(json!({
        "ok": logout_request_error.is_empty() && verify_error.is_empty(),
        "logged_out": logged_out,
        "message": message,
        "logout_request_error": logout_request_error,
        "verify_error": verify_error,
        "verify": {
            "endpoint": "/account/info?fr=pc&platform=pc",
            "status_code": account.status_code,
            "code": account.code,
            "message": account.message,
            "nickname": account.nickname,
        }
    }))
}
