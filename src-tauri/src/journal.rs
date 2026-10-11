//! 转存文件账本（票 11）：记录每次转存进用户网盘的 fid，删除成功后销账；
//! 应用启动时清理超过 1 小时的滞留条目（正常流程几秒内就会删掉，
//! 滞留即意味着上次会话中途崩溃——正是要兜底的场景）。

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Mutex;

static PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub fid: String,
    /// 记账时间（unix 秒）
    pub ts: i64,
}

pub fn init(config_dir: Option<PathBuf>) {
    if let Some(dir) = config_dir {
        let _ = std::fs::create_dir_all(&dir);
        *PATH.lock().unwrap() = Some(dir.join("save-journal.json"));
    }
}

fn load() -> Vec<Entry> {
    let Some(path) = PATH.lock().unwrap().clone() else { return Vec::new() };
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save(entries: &[Entry]) {
    let Some(path) = PATH.lock().unwrap().clone() else { return };
    if let Ok(s) = serde_json::to_string_pretty(entries) {
        let _ = std::fs::write(&path, s);
    }
}

/// 转存成功后记账
pub fn record(fid: &str) {
    if fid.is_empty() {
        return;
    }
    let mut entries = load();
    if entries.iter().any(|e| e.fid == fid) {
        return;
    }
    entries.push(Entry {
        fid: fid.to_string(),
        ts: now_secs(),
    });
    save(&entries);
}

/// 删除转存文件成功后销账
pub fn remove(fid: &str) {
    let mut entries = load();
    let before = entries.len();
    entries.retain(|e| e.fid != fid);
    if entries.len() != before {
        save(&entries);
    }
}

/// 取出滞留超时的账目（不销账——销账由调用方在删除成功后做）
pub fn stale(max_age_secs: i64) -> Vec<Entry> {
    let cutoff = now_secs() - max_age_secs;
    load()
        .into_iter()
        .filter(|e| e.ts <= cutoff)
        .collect()
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 启动清理：对滞留账目逐个调用删除接口；成功即销账。
/// 未登录时跳过（下次启动再试，账本不丢）。
pub async fn cleanup_stale() -> Result<usize, String> {
    let stale_entries = stale(3600);
    if stale_entries.is_empty() {
        return Ok(0);
    }
    let jar = crate::cookie_store::snapshot().unwrap_or_default();
    if jar.is_empty() {
        tracing::warn!(count = stale_entries.len(), "发现滞留转存记录，但当前未登录，推迟到下次启动清理");
        return Ok(0);
    }

    let routes = quark_core::protocol::get_api_routes();
    let route = routes.get("/api/file/delete").unwrap();
    let mut cleaned = 0usize;
    for entry in &stale_entries {
        let body = serde_json::to_vec(&json!({
            "action_type": 2,
            "filelist": [entry.fid],
            "exclude_fids": [],
        }))
        .unwrap();
        match quark_core::protocol::proxy_request(
            route.path,
            route.method.clone(),
            Some(&jar),
            Some(body.into()),
            "",
            route.host,
            true,
        )
        .await
        {
            Ok(resp) => {
                cookie_store_merge(&resp);
                if resp.status == 200 {
                    remove(&entry.fid);
                    cleaned += 1;
                }
            }
            Err(_) => {}
        }
    }
    Ok(cleaned)
}

fn cookie_store_merge(resp: &quark_core::protocol::ProxyResponse) {
    let values = crate::cookie_store::collect_set_cookies(&resp.headers);
    crate::cookie_store::update_from_set_cookie(&values);
}

#[tauri::command]
pub async fn journal_cleanup() -> Result<serde_json::Value, String> {
    let n = cleanup_stale().await?;
    Ok(json!({ "cleaned": n }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_journal(name: &str) {
        let dir = std::env::temp_dir().join(format!("journal-test-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        init(Some(dir));
    }

    // 全局 PATH 静态态——两个用例合并为顺序步骤避免并行竞态
    #[test]
    fn journal_lifecycle() {
        test_journal("lifecycle");
        assert_eq!(stale(3600).len(), 0);
        record("fid-1");
        record("fid-2");
        record("fid-1"); // 重复记账幂等
        assert_eq!(load_pub().len(), 2);
        remove("fid-1");
        assert_eq!(load_pub().iter().map(|e| e.fid.clone()).collect::<Vec<_>>(), vec!["fid-2".to_string()]);
        // 刚记的账不可能滞留 1 小时
        assert!(stale(3600).is_empty());
        // 时间回拨到 2 小时前 → 视为滞留
        let mut entries = load_pub();
        entries[0].ts = now_secs() - 7200;
        save(&entries);
        assert_eq!(stale(3600).len(), 1);
    }

    fn load_pub() -> Vec<Entry> {
        load()
    }
}
