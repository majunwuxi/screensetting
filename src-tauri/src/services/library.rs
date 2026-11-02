//! 本地图片库：缩略图、历史记录装饰与体积控制。

use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Paths;
use crate::imaging;
use crate::models::HistoryEntry;

/// 给历史记录补上可直接显示的缩略图（base64）。
pub fn history_with_thumbs(paths: &Paths, history: &[HistoryEntry]) -> Vec<HistoryEntry> {
    let _ = paths;
    history
        .iter()
        .take(60)
        .map(|entry| {
            let mut item = entry.clone();
            let candidate = entry
                .thumb_path
                .as_ref()
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .or_else(|| Some(PathBuf::from(&entry.wallpaper_path)));

            if let Some(path) = candidate {
                if path.exists() {
                    if let Ok((data_url, _, _)) = imaging::preview_data_url(&path, 420, 72) {
                        item.thumb = Some(data_url);
                    }
                }
            }
            item
        })
        .collect()
}

/// 仅保留最近 `keep` 个文件，避免图片库无限膨胀。
pub fn prune_dir(dir: &Path, keep: usize) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = read_dir
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            Some((modified, path))
        })
        .collect();

    files.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path) in files.into_iter().skip(keep) {
        let _ = fs::remove_file(path);
    }
}

pub fn prune_all(paths: &Paths) {
    prune_dir(&paths.library, 60);
    prune_dir(&paths.thumbs, 120);
}

pub fn library_file_count(paths: &Paths) -> usize {
    fs::read_dir(&paths.library)
        .map(|dir| dir.flatten().filter(|e| e.path().is_file()).count())
        .unwrap_or(0)
}
