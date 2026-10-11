//! 设置：下载目录、记住上次目录、同名策略等。JSON 持久化在应用配置目录（票 06）。

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);
static CONFIG_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// 用户固定的下载目录；None = 系统默认下载目录
    pub download_dir: Option<String>,
    /// 最近一次选择的目录
    pub last_dir: Option<String>,
    /// 记住上次目录：开启时，最近一次选择在下个会话继续生效
    pub remember_last_dir: bool,
    /// rename | skip | ask
    pub duplicate_strategy: String,
    /// 深浅主题：system | light | dark（票 09）
    pub appearance: String,
    /// 任务完成后是否弹完成态小窗（票 08）
    pub progress_window_on_completion: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            download_dir: None,
            last_dir: None,
            remember_last_dir: true,
            duplicate_strategy: "rename".into(),
            appearance: "system".into(),
            progress_window_on_completion: true,
        }
    }
}

pub fn init(config_dir: Option<PathBuf>) {
    let Some(dir) = config_dir else {
        *SETTINGS.lock().unwrap() = Some(Settings::default());
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("settings.json");
    let loaded = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<Settings>(&s).ok());
    *CONFIG_PATH.lock().unwrap() = Some(path);
    *SETTINGS.lock().unwrap() = Some(loaded.unwrap_or_default());
}

pub fn get() -> Settings {
    SETTINGS
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_default()
}

fn persist(settings: &Settings) {
    if let Some(path) = CONFIG_PATH.lock().unwrap().clone() {
        if let Ok(s) = serde_json::to_string_pretty(settings) {
            if let Err(e) = std::fs::write(&path, s) {
                tracing::warn!("写入失败: {e}");
            }
        }
    }
}

pub fn update(patch: SettingsPatch) -> Settings {
    let mut s = get();
    if let Some(v) = patch.download_dir {
        s.last_dir = Some(v.clone());
        if s.remember_last_dir {
            s.download_dir = Some(v);
        }
    }
    if let Some(v) = patch.remember_last_dir {
        s.remember_last_dir = v;
    }
    if let Some(v) = patch.duplicate_strategy {
        if matches!(v.as_str(), "rename" | "skip" | "ask") {
            s.duplicate_strategy = v;
        }
    }
    if let Some(v) = patch.appearance {
        if matches!(v.as_str(), "system" | "light" | "dark") {
            s.appearance = v;
        }
    }
    if let Some(v) = patch.progress_window_on_completion {
        s.progress_window_on_completion = v;
    }
    persist(&s);
    *SETTINGS.lock().unwrap() = Some(s.clone());
    s
}

/// 实际生效的下载目录
pub fn effective_dir() -> PathBuf {
    let s = get();
    if let Some(dir) = s.download_dir.or_else(|| s.last_dir.filter(|_| s.remember_last_dir)) {
        let p = PathBuf::from(&dir);
        if p.is_absolute() {
            return p;
        }
    }
    dirs::download_dir().unwrap_or_else(|| PathBuf::from("."))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsPatch {
    pub download_dir: Option<String>,
    pub remember_last_dir: Option<bool>,
    pub duplicate_strategy: Option<String>,
    pub appearance: Option<String>,
    pub progress_window_on_completion: Option<bool>,
}

impl Default for SettingsPatch {
    fn default() -> Self {
        Self {
            download_dir: None,
            remember_last_dir: None,
            duplicate_strategy: None,
            appearance: None,
            progress_window_on_completion: None,
        }
    }
}

// ── Tauri 命令 ───────────────────────────────────────────

#[tauri::command]
pub async fn settings_get() -> Result<Settings, String> {
    Ok(get())
}

#[tauri::command]
pub async fn settings_set(patch: SettingsPatch) -> Result<Settings, String> {
    Ok(update(patch))
}

/// 传入即将下载的文件名，返回其中与现有文件重名的
#[tauri::command]
pub async fn check_duplicates(names: Vec<String>) -> Result<Vec<String>, String> {
    let dir = effective_dir();
    Ok(names
        .into_iter()
        .filter(|n| dir.join(n).exists())
        .collect())
}

/// 在系统文件管理器中定位文件
#[tauri::command]
pub async fn open_folder(path: String) -> Result<Value, String> {
    let path2 = path.clone();
    std::thread::spawn(move || {
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer")
                .arg(format!("/select,{}", path2))
                .spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").args(["-R", &path2]).spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let parent = std::path::Path::new(&path2)
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_default();
            let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
        }
    });
    Ok(json!({ "ok": true }))
}

/// 用系统默认程序打开文件
#[tauri::command]
pub async fn open_file(path: String) -> Result<Value, String> {
    std::thread::spawn(move || {
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer").arg(&path).spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg(&path).spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
        }
    });
    Ok(json!({ "ok": true }))
}

/// 系统目录选择器（rfd 阻塞调用，放独立线程）
#[tauri::command]
pub async fn pick_folder() -> Result<Option<String>, String> {
    let initial = get()
        .last_dir
        .or_else(|| get().download_dir);
    tokio::task::spawn_blocking(move || {
        let mut dialog = rfd::FileDialog::new();
        if let Some(dir) = initial {
            let p = PathBuf::from(&dir);
            if p.is_dir() {
                dialog = dialog.set_directory(p);
            }
        }
        Ok(dialog.pick_folder().map(|p| p.to_string_lossy().to_string()))
    })
    .await
    .map_err(|e| format!("目录选择器失败: {e}"))?
}
