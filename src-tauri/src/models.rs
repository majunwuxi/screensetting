//! 与前端交换的数据结构。

use serde::{Deserialize, Serialize};

/// 一张 Unsplash 照片（已裁剪为前端需要的字段）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Photo {
    pub id: String,
    pub description: Option<String>,
    pub width: u32,
    pub height: u32,
    pub color: Option<String>,
    pub author: String,
    pub author_url: Option<String>,
    pub thumb_url: String,
    pub preview_url: String,
    pub full_url: String,
    pub unsplash_url: Option<String>,
    pub download_location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub query: String,
    pub page: u32,
    pub total: u32,
    pub total_pages: u32,
    pub photos: Vec<Photo>,
}

/// 显示器 / 桌面拓扑信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenInfo {
    /// 主显示器物理像素宽度。
    pub width: u32,
    /// 主显示器物理像素高度。
    pub height: u32,
    pub virtual_width: u32,
    pub virtual_height: u32,
    pub monitors: u32,
    pub is_windows: bool,
}

/// 本地图片预览（base64 data URL，避免前端直接读取文件系统）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewImage {
    pub data_url: String,
    pub path: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub size: u64,
}

/// 应用历史（同时持久化到 history.json）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub title: String,
    /// `unsplash` | `local`
    pub source: String,
    /// 原始地址（远程 URL 或本地路径）。
    pub origin: String,
    pub wallpaper_path: String,
    /// 屏幕尺寸的图片文件（用于重新应用 / 打开文件夹）。
    pub thumb_path: Option<String>,
    pub applied_at: i64,
    pub fit: String,
    pub resolution: String,
    pub lock_target: bool,
    /// 仅在返回给前端时填充的缩略预览。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumb: Option<String>,
}

/// 图片来源于哪里。
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ImageSource {
    /// 来自 Unsplash 搜索结果。
    Remote {
        id: String,
        url: String,
        title: Option<String>,
        download_location: Option<String>,
    },
    /// 来自本地文件。
    Local { path: String, title: Option<String> },
    /// 来自历史记录里的已渲染文件。
    History {
        id: String,
        path: String,
        title: Option<String>,
        origin: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyRequest {
    pub source: ImageSource,
    /// cover | contain | stretch | center | tile | blur
    pub fit: String,
    #[serde(default)]
    pub wallpaper: bool,
    #[serde(default)]
    pub lockscreen: bool,
    pub quality: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepResult {
    pub success: bool,
    pub message: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyReport {
    pub ok: bool,
    pub needs_admin: bool,
    pub wallpaper: Option<StepResult>,
    pub lockscreen: Option<StepResult>,
    pub rendered_path: Option<String>,
    pub exported_path: Option<String>,
    pub message: String,
    pub history: Option<HistoryEntry>,
}

/// 应用启动时一次性下发的状态。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub settings: crate::config::Settings,
    pub screen: ScreenInfo,
    pub is_admin: bool,
    pub history: Vec<HistoryEntry>,
    pub version: String,
    pub cache_bytes: u64,
    pub library_count: usize,
    pub system: SystemSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSummary {
    pub os: String,
    pub webview: String,
    pub default_wallpaper: String,
    pub lockscreen_dir: String,
}
