//! 设置与历史的持久化。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::models::HistoryEntry;

/// 与旧版 Python 程序保持一致的 Unsplash 访问密钥，用户可在设置里替换。
pub const DEFAULT_UNSPLASH_KEY: &str = "PtWrZwThqF0NYvw_XDTKwc4YOrRNWevrZQYfvJ02nLE";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub unsplash_key: String,
    pub download_dir: String,
    /// cover | contain | stretch | center | tile | blur
    pub default_fit: String,
    pub jpeg_quality: u8,
    /// 是否把每次应用的成图保留在应用图片库中（便于重新应用）。
    pub keep_library_copy: bool,
    /// 默认同时设置锁屏。
    pub default_lockscreen: bool,
    /// 应用成功后自动清理 30 天前的历史成图。
    pub auto_prune: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            unsplash_key: DEFAULT_UNSPLASH_KEY.to_string(),
            download_dir: String::new(),
            default_fit: "cover".to_string(),
            jpeg_quality: 92,
            keep_library_copy: true,
            default_lockscreen: true,
            auto_prune: true,
        }
    }
}

impl Settings {
    pub fn sanitized(mut self) -> Self {
        if self.unsplash_key.trim().is_empty() {
            self.unsplash_key = DEFAULT_UNSPLASH_KEY.to_string();
        }
        const FITS: [&str; 6] = ["cover", "contain", "stretch", "center", "tile", "blur"];
        if !FITS.contains(&self.default_fit.as_str()) {
            self.default_fit = "cover".to_string();
        }
        self.jpeg_quality = self.jpeg_quality.clamp(60, 100);
        self
    }
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub data: PathBuf,
    pub cache: PathBuf,
    pub library: PathBuf,
    pub thumbs: PathBuf,
    pub settings_file: PathBuf,
    pub history_file: PathBuf,
}

impl Paths {
    pub fn new(data: PathBuf, cache: PathBuf) -> Self {
        let library = data.join("library");
        let thumbs = data.join("thumbs");
        Self {
            settings_file: data.join("settings.json"),
            history_file: data.join("history.json"),
            data,
            cache,
            library,
            thumbs,
        }
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        fs::create_dir_all(&self.data)?;
        fs::create_dir_all(&self.cache)?;
        fs::create_dir_all(&self.library)?;
        fs::create_dir_all(&self.thumbs)?;
        Ok(())
    }
}

pub fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建目录失败：{e}"))?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| format!("序列化失败：{e}"))?;
    fs::write(path, bytes).map_err(|e| format!("写入配置失败：{e}"))
}

pub fn load_settings(paths: &Paths) -> Settings {
    read_json::<Settings>(&paths.settings_file)
        .unwrap_or_default()
        .sanitized()
}

pub fn save_settings(paths: &Paths, settings: &Settings) -> Result<(), String> {
    write_json(&paths.settings_file, settings)
}

pub fn load_history(paths: &Paths) -> Vec<HistoryEntry> {
    let mut list: Vec<HistoryEntry> = read_json(&paths.history_file).unwrap_or_default();
    list.truncate(120);
    list
}

pub fn save_history(paths: &Paths, history: &[HistoryEntry]) -> Result<(), String> {
    let cleaned: Vec<HistoryEntry> = history
        .iter()
        .cloned()
        .map(|mut e| {
            e.thumb = None;
            e
        })
        .collect();
    write_json(&paths.history_file, &cleaned)
}

/// 递归统计一个目录的占用大小。
pub fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            total += dir_size(&entry.path());
        } else {
            total += meta.len();
        }
    }
    total
}
