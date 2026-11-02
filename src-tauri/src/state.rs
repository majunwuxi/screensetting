//! 应用运行期状态。

use std::sync::Mutex;
use std::time::Duration;

use tauri::Manager;

use crate::config::{self, Paths, Settings};
use crate::models::HistoryEntry;

pub struct AppState {
    pub paths: Paths,
    pub client: reqwest::Client,
    pub settings: Mutex<Settings>,
    pub history: Mutex<Vec<HistoryEntry>>,
}

impl AppState {
    pub fn new(app: &tauri::AppHandle) -> Result<Self, String> {
        let data = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("无法定位应用数据目录：{e}"))?;
        let cache = app
            .path()
            .app_cache_dir()
            .map_err(|e| format!("无法定位缓存目录：{e}"))?
            // 只把应用自己的临时文件放进子目录：缓存根目录还存放着 WebView2 的
            // 用户数据（EBWebView），直接清空会导致 WebView2 文件被占用而失败。
            .join("studio-cache");
        let pictures = app
            .path()
            .picture_dir()
            .unwrap_or_else(|_| data.clone());

        let paths = Paths::new(data, cache);
        paths
            .ensure()
            .map_err(|e| format!("初始化目录失败：{e}"))?;

        let mut settings = config::load_settings(&paths);
        if settings.download_dir.trim().is_empty() {
            settings.download_dir = pictures.join("Wallpapers").display().to_string();
        }
        let history = config::load_history(&paths);

        let client = reqwest::Client::builder()
            .user_agent("WallpaperStudio/2.0 (Windows NT 10.0; Win64; x64)")
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| format!("初始化网络客户端失败：{e}"))?;

        Ok(Self {
            paths,
            client,
            settings: Mutex::new(settings),
            history: Mutex::new(history),
        })
    }

    pub fn settings_snapshot(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }
}
