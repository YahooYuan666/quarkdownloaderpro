//! Cookie 生命周期：解析、合并（同名键覆盖）、序列化。
//!
//! 修复上游 P0（R1.1）：上游 `http.ts:22` 只在缺失 `__puus` 时才写回服务端
//! 下发的刷新值，导致会话令牌永不更新。本模块的 `merge_response` 对同名键
//! 一律覆盖——服务端下发什么，存储里就是什么。

/// 有序键值对。顺序保持「首次出现的键位固定、值取最新」，
/// 与浏览器 Cookie 语义一致，也便于稳定测试。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CookieJar {
    pairs: Vec<(String, String)>,
}

impl CookieJar {
    /// 解析 `Cookie:` 请求头形态的字符串："a=1; b=2"。
    /// 同名键后者覆盖前者；无 `=` 的片段忽略；值中允许包含 `=`。
    pub fn parse(header: &str) -> Self {
        let mut jar = Self::default();
        for pair in header.split(';') {
            let pair = pair.trim();
            if pair.is_empty() {
                continue;
            }
            if let Some(eq) = pair.find('=') {
                let name = pair[..eq].trim();
                if name.is_empty() {
                    continue;
                }
                let value = pair[eq + 1..].trim().to_string();
                jar.set(name, value);
            }
        }
        jar
    }

    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        let value = value.into();
        if let Some(slot) = self.pairs.iter_mut().find(|(k, _)| *k == name) {
            slot.1 = value;
        } else {
            self.pairs.push((name, value));
        }
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn contains_key(&self, name: &str) -> bool {
        self.pairs.iter().any(|(k, _)| k == name)
    }

    /// 合并 `Set-Cookie` 响应头的原始值（如 "__puus=xxx; Path=/; HttpOnly"）。
    /// 取每个值的首个 `;` 前的键值对，同名键**覆盖**——这是会话令牌轮换的
    /// 关键路径（R1.1）。
    pub fn merge_response(&mut self, set_cookie_values: &[String]) {
        for raw in set_cookie_values {
            let pair = raw.split(';').next().unwrap_or("").trim();
            if pair.is_empty() {
                continue;
            }
            if let Some(eq) = pair.find('=') {
                let name = pair[..eq].trim();
                if name.is_empty() {
                    continue;
                }
                let value = pair[eq + 1..].trim().to_string();
                self.set(name, value);
            }
        }
    }

    /// 序列化为 `Cookie:` 请求头。
    pub fn header(&self) -> String {
        self.pairs
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ")
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.pairs.len()
    }
}

/// 账号信息有效性判定——上游 `parse_account_info_state` 与
/// `can_read_account_info` 的统一实现（上游写了两遍，此处收敛为一）。
pub fn account_state_is_logged_in(status: u16, payload: &serde_json::Value) -> bool {
    let code_text = payload
        .get("code")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_uppercase();
    let message_text = payload
        .get("message")
        .or_else(|| payload.get("msg"))
        .or_else(|| payload.get("error"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let data = payload.get("data");
    let has_identity = !data
        .and_then(|d| d.get("nickname"))
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .trim()
        .is_empty()
        || data
            .and_then(|d| d.get("mobilekps"))
            .and_then(|v| v.as_str())
            .map_or(false, |s| !s.is_empty());

    let success =
        payload.get("success").and_then(|v| v.as_bool()).unwrap_or(false) || code_text == "OK";

    let combined = format!("{} {}", message_text, code_text).to_lowercase();
    let auth_error = message_text.contains("未登录")
        || message_text.contains("登录失效")
        || combined.contains("invalid")
        || combined.contains("expired")
        || combined.contains("unauthorized")
        || combined.contains("forbidden");

    (200..300).contains(&status) && success && has_identity && !auth_error
}

/// 上游响应是否为登录态失效（用于触发「重新登录指引」事件）
pub fn is_auth_failure(payload: &serde_json::Value) -> bool {
    let message = payload
        .get("message")
        .or_else(|| payload.get("msg"))
        .or_else(|| payload.get("error"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let code = payload
        .get("code")
        .map(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| v.to_string())
        })
        .unwrap_or_default();
    let combined = format!("{} {}", message, code).to_lowercase();
    message.contains("未登录")
        || message.contains("登录失效")
        || combined.contains("unauthorized")
        || combined.contains("forbidden")
        || combined.contains("not login")
        || combined.contains("token expire")
        || combined.contains("session expire")
}

pub fn account_nickname(payload: &serde_json::Value) -> String {
    payload
        .get("data")
        .and_then(|d| d.get("nickname"))
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_roundtrip_and_dedupe() {
        let jar = CookieJar::parse("a=1; b=2; a=3");
        assert_eq!(jar.header(), "a=3; b=2");
        assert_eq!(jar.get("b"), Some("2"));
        assert_eq!(jar.len(), 2);
    }

    #[test]
    fn value_may_contain_equals() {
        let jar = CookieJar::parse("k=v=1=2; b=x");
        assert_eq!(jar.get("k"), Some("v=1=2"));
    }

    #[test]
    fn merge_response_overwrites_existing_token() {
        // R1.1 核心剧本：已有 __puus，服务端下发新值 → 必须覆盖
        let mut jar = CookieJar::parse("__pus=old_pus; __puus=OLD; foo=bar");
        jar.merge_response(&[
            "__puus=NEW; Path=/; Domain=quark.cn; HttpOnly".to_string(),
        ]);
        assert_eq!(jar.get("__puus"), Some("NEW"));
        assert_eq!(jar.get("__pus"), Some("old_pus"));
        assert_eq!(jar.get("foo"), Some("bar"));
    }

    #[test]
    fn merge_response_adds_new_keys() {
        let mut jar = CookieJar::parse("a=1");
        jar.merge_response(&["b=2; Path=/".to_string(), "c=3".to_string()]);
        assert_eq!(jar.header(), "a=1; b=2; c=3");
    }

    #[test]
    fn merge_ignores_garbage() {
        let mut jar = CookieJar::parse("a=1");
        jar.merge_response(&[
            String::new(),
            "; Path=/".to_string(),
            "=novalue".to_string(),
        ]);
        assert_eq!(jar.header(), "a=1");
    }

    #[test]
    fn account_state_rejects_logged_out() {
        let payload = serde_json::json!({"code": "LOGGED_OUT", "message": "未登录"});
        assert!(!account_state_is_logged_in(200, &payload));
        let ok = serde_json::json!({"success": true, "data": {"nickname": "小明"}});
        assert!(account_state_is_logged_in(200, &ok));
        assert_eq!(account_nickname(&ok), "小明");
    }
}
