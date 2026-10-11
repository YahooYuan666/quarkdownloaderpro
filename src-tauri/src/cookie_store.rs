//! Cookie 存储：内存态 CookieJar + 系统凭据库持久化（Windows 凭据管理器 /
//! macOS Keychain / Linux Secret Service）。
//!
//! R6.3 / ADR-0001：前端不再持有明文 Cookie，浏览器 localStorage 彻底退出；
//! 持久化失败时降级为纯内存态（应用仍可用，重启后需重新登录），并记录警告日志。

use quark_core::CookieJar;
use std::sync::Mutex;

static JAR: Mutex<Option<CookieJar>> = Mutex::new(None);

const SERVICE: &str = "QuarkDownloaderPro";
const ACCOUNT: &str = "quark_cookie";

/// 凭据库是否可用（启动时探测一次，供界面提示）
static STORE_AVAILABLE: Mutex<Option<bool>> = Mutex::new(None);

fn entry() -> Option<keyring::Entry> {
    match keyring::Entry::new(SERVICE, ACCOUNT) {
        Ok(e) => {
            *STORE_AVAILABLE.lock().unwrap() = Some(true);
            Some(e)
        }
        Err(e) => {
            // 这里曾经静默返回，导致"每次启动都要重新登录"却查不出原因——
            // 凭据库后端缺失是运行期才暴露的，必须留下痕迹。
            *STORE_AVAILABLE.lock().unwrap() = Some(false);
            tracing::error!("系统凭据库不可用，登录状态将无法保存：{e}");
            None
        }
    }
}

/// 供界面查询：凭据库是否可用（false 时说明"记住登录"会失效）
pub fn storage_available() -> bool {
    STORE_AVAILABLE.lock().unwrap().unwrap_or(false)
}

/// 应用启动时调用：从系统凭据库恢复会话
pub fn init() {
    let Some(entry) = entry() else {
        tracing::warn!("凭据库不可用，本次会话 Cookie 仅存内存");
        return;
    };
    match entry.get_password() {
        Ok(saved) if !saved.is_empty() => {
            *JAR.lock().unwrap() = Some(CookieJar::parse(&saved));
            tracing::info!("已从系统凭据库恢复会话（{} 个字段）", CookieJar::parse(&saved).len());
        }
        Ok(_) => {}
        Err(keyring::Error::NoEntry) => {}
        Err(e) => eprintln!("[cookie-store] 读取凭据库失败: {e}"),
    }
}

/// 当前 Cookie 请求头（无会话时为空串）
pub fn header() -> String {
    JAR.lock()
        .unwrap()
        .as_ref()
        .map(|j| j.header())
        .unwrap_or_default()
}

/// 是否已有会话
pub fn has_session() -> bool {
    JAR.lock().unwrap().as_ref().map(|j| !j.is_empty()).unwrap_or(false)
}

/// 保存用户粘贴的 Cookie 文本
pub fn set_from_text(text: &str) -> CookieJar {
    let jar = CookieJar::parse(text);
    persist(&jar);
    *JAR.lock().unwrap() = Some(jar.clone());
    jar
}

/// 合并服务端下发的 Set-Cookie（同名键覆盖——R1.1 修复的唯一写入路径）
pub fn update_from_set_cookie(values: &[String]) {
    if values.is_empty() {
        return;
    }
    let mut guard = JAR.lock().unwrap();
    let jar = guard.get_or_insert_with(CookieJar::default);
    jar.merge_response(values);
    persist(jar);
}

/// 清除会话（本地 + 远端凭据）
pub fn clear() {
    *JAR.lock().unwrap() = None;
    if let Some(entry) = entry() {
        let _ = entry.delete_credential();
    }
}

fn persist(jar: &CookieJar) {
    let Some(entry) = entry() else { return };
    let header = jar.header();
    let result = if header.is_empty() {
        entry.delete_credential()
    } else {
        entry.set_password(&header)
    };
    match result {
        Ok(_) => tracing::info!("登录凭据已写入系统凭据库"),
        Err(e) => tracing::error!("写入凭据库失败，本次仅内存态（重启需重新登录）: {e}"),
    }
}

/// 从响应头收集全部 Set-Cookie 原始值（api/tasks/journal 共用，避免三处重复）
pub fn collect_set_cookies(headers: &reqwest::header::HeaderMap) -> Vec<String> {
    headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .collect()
}

/// 当前会话的 CookieJar 快照（供协议层调用）
pub fn snapshot() -> Option<CookieJar> {
    JAR.lock().unwrap().clone()
}

#[cfg(test)]
mod tests {
    /// Linux CI（headless runner）通常没有解锁的 Secret Service，此时跳过而不是报错；
    /// Windows/macOS 恒有原生凭据库，任何失败都视为回归。
    /// 探针用读操作：存储可达返回 Ok/NoEntry；不可达返回 NoStorageAccess/PlatformFailure。
    fn skip_if_no_store(entry: &keyring::Entry) -> bool {
        if !cfg!(target_os = "linux") {
            return false;
        }
        match entry.get_password() {
            Ok(_) | Err(keyring::Error::NoEntry) => false,
            Err(keyring::Error::NoStorageAccess(_) | keyring::Error::PlatformFailure(_)) => {
                eprintln!("跳过：当前 Linux 环境没有可用的 Secret Service（headless CI 属预期）");
                true
            }
            Err(e) => panic!("凭据库探针失败：{e}"),
        }
    }

    /// 直接验证系统凭据库能读能写。
    /// 这条测试的意义：keyring 的平台后端是“选择性启用”的，缺特性时
    /// Entry::new() 会在运行期失败——本项目曾因此静默地每次启动都要求重新登录。
    #[test]
    fn keyring_roundtrip() {
        let entry = keyring::Entry::new("QuarkDownloaderProSelfTest", "roundtrip")
            .unwrap_or_else(|e| panic!("系统凭据库不可用：平台后端特性可能未启用：{e}"));
        if skip_if_no_store(&entry) {
            return;
        }
        entry.set_password("probe-value").expect("写入凭据库失败");
        let back = entry.get_password().expect("读取凭据库失败");
        assert_eq!(back, "probe-value");
        let _ = entry.delete_credential();
    }

    /// 验证**生产用的那个凭据槽位**能读写（上面那条只测了一个独立槽位）。
    /// 会先备份再还原，不会破坏已登录状态。
    #[test]
    fn app_credential_slot_roundtrip() {
        let entry = keyring::Entry::new(super::SERVICE, super::ACCOUNT)
            .unwrap_or_else(|e| panic!("凭据库不可用：{e}"));
        if skip_if_no_store(&entry) {
            return;
        }
        let backup = entry.get_password().ok();
        entry.set_password("__self_test__").expect("写入失败");
        assert_eq!(entry.get_password().unwrap(), "__self_test__");
        match backup {
            Some(v) => entry.set_password(&v).expect("还原失败"),
            None => { let _ = entry.delete_credential(); }
        }
    }
}
