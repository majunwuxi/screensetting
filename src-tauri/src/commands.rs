//! 暴露给前端的 Tauri 命令。

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::config::{self, Settings};
use crate::models::{
    ApplyReport, ApplyRequest, Bootstrap, HistoryEntry, ImageSource, PreviewImage, ScreenInfo,
    SearchResponse, StepResult, SystemSummary,
};
use crate::state::AppState;
use crate::{imaging, lockscreen, services, sysinfo, wallpaper};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn paths_of(app: &AppHandle) -> Result<config::Paths, String> {
    Ok(app.state::<AppState>().paths.clone())
}

fn file_name_of(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "本地图片".to_string())
}

fn preview_from_path(path: &Path, max_edge: u32, quality: u8) -> Result<PreviewImage, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("无法读取文件信息：{e}"))?;
    if !meta.is_file() {
        return Err("请选择一个图片文件。".to_string());
    }
    let (data_url, width, height) = imaging::preview_data_url(path, max_edge, quality)?;
    Ok(PreviewImage {
        data_url,
        path: path.display().to_string(),
        name: path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| file_name_of(path)),
        width,
        height,
        size: meta.len(),
    })
}

#[tauri::command]
pub async fn bootstrap(app: AppHandle) -> Result<Bootstrap, String> {
    let (paths, settings, history, cache_bytes) = {
        let state = app.state::<AppState>();
        let paths = state.paths.clone();
        let settings = state.settings_snapshot();
        let history = state.history.lock().map_err(|e| e.to_string())?.clone();
        let cache_bytes = config::dir_size(&state.paths.cache);
        (paths, settings, history, cache_bytes)
    };

    let lib_paths = paths.clone();
    let decorated = tauri::async_runtime::spawn_blocking(move || {
        services::library::history_with_thumbs(&lib_paths, &history)
    })
    .await
    .map_err(|e| format!("加载历史记录失败：{e}"))?;

    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());

    Ok(Bootstrap {
        settings,
        screen: sysinfo::screen_info(),
        is_admin: sysinfo::is_admin(),
        history: decorated,
        version: VERSION.to_string(),
        cache_bytes,
        library_count: services::library::library_file_count(&paths),
        system: SystemSummary {
            os: sysinfo::os_caption(),
            webview: "Microsoft Edge WebView2".to_string(),
            default_wallpaper: PathBuf::from(&system_root)
                .join(r"Web\Wallpaper\Windows\img0.jpg")
                .display()
                .to_string(),
            lockscreen_dir: PathBuf::from(&system_root)
                .join(r"Web\Screen")
                .display()
                .to_string(),
        },
    })
}

#[tauri::command]
pub async fn search_photos(
    app: AppHandle,
    query: String,
    page: Option<u32>,
    per_page: Option<u32>,
) -> Result<SearchResponse, String> {
    let trimmed = query.trim().to_string();
    if trimmed.is_empty() {
        return Err("请输入搜索关键词。".to_string());
    }

    let (client, key) = {
        let state = app.state::<AppState>();
        let settings = state.settings_snapshot();
        (state.client.clone(), settings.unsplash_key)
    };

    services::unsplash::search(
        &client,
        &key,
        &trimmed,
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(24).clamp(6, 30),
    )
    .await
}

#[tauri::command]
pub async fn preview_image(app: AppHandle, path: String, max_edge: Option<u32>) -> Result<PreviewImage, String> {
    let _ = app;
    let edge = max_edge.unwrap_or(1800).clamp(320, 2600);
    tauri::async_runtime::spawn_blocking(move || {
        preview_from_path(Path::new(&path), edge, 88)
    })
    .await
    .map_err(|e| format!("生成预览失败：{e}"))?
}

#[tauri::command]
pub async fn pick_image(max_edge: Option<u32>) -> Result<Option<PreviewImage>, String> {
    let picked = tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .add_filter(
                "图片文件",
                &["jpg", "jpeg", "png", "webp", "bmp", "gif", "tif", "tiff"],
            )
            .set_title("选择本地图片")
            .pick_file()
    })
    .await
    .map_err(|e| format!("打开文件选择器失败：{e}"))?;

    match picked {
        Some(path) => {
            let edge = max_edge.unwrap_or(1800).clamp(320, 2600);
            tauri::async_runtime::spawn_blocking(move || preview_from_path(&path, edge, 88))
                .await
                .map_err(|e| format!("生成预览失败：{e}"))?
                .map(Some)
        }
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn pick_folder() -> Result<Option<String>, String> {
    let picked = tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("选择图片保存目录")
            .pick_folder()
    })
    .await
    .map_err(|e| format!("打开目录选择器失败：{e}"))?;
    Ok(picked.map(|p| p.display().to_string()))
}

fn safe_id(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(48)
        .collect()
}

#[tauri::command]
pub async fn apply_background(app: AppHandle, request: ApplyRequest) -> Result<ApplyReport, String> {
    if !request.wallpaper && !request.lockscreen {
        return Err("请至少选择一个应用目标（桌面壁纸或锁屏）。".to_string());
    }

    let (paths, client, settings) = {
        let state = app.state::<AppState>();
        (
            state.paths.clone(),
            state.client.clone(),
            state.settings_snapshot(),
        )
    };

    let quality = request
        .quality
        .unwrap_or(settings.jpeg_quality)
        .clamp(60, 100);
    let fit = match request.fit.as_str() {
        "contain" | "stretch" | "center" | "tile" | "blur" => request.fit.clone(),
        _ => "cover".to_string(),
    };
    let screen = sysinfo::screen_info();

    // 1) 解析来源，必要时下载。
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let (source_path, origin, title, source_kind) = match &request.source {
        ImageSource::Remote {
            id,
            url,
            title,
            download_location,
        } => {
            let dest = paths
                .library
                .join(format!("unsplash-{}.jpg", safe_id(id)));
            let needs_download = !dest.exists()
                || std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0) < 1024;
            if needs_download {
                let target = services::unsplash::sized_url(url, 3840);
                services::unsplash::download(&client, &target, &dest).await?;
            }
            if let Some(location) = download_location {
                services::unsplash::track_download(
                    client.clone(),
                    settings.unsplash_key.clone(),
                    location.clone(),
                );
            }
            (
                dest,
                url.clone(),
                title.clone().unwrap_or_else(|| "Unsplash 作品".to_string()),
                "unsplash".to_string(),
            )
        }
        ImageSource::Local { path, title } => {
            let path_buf = PathBuf::from(path);
            if !path_buf.exists() {
                return Err("本地图片不存在或已被移动。".to_string());
            }
            (
                path_buf.clone(),
                path.clone(),
                title
                    .clone()
                    .unwrap_or_else(|| file_name_of(&path_buf)),
                "local".to_string(),
            )
        }
        ImageSource::History {
            id,
            path,
            title,
            origin,
        } => {
            let _ = id;
            let path_buf = PathBuf::from(path);
            if !path_buf.exists() {
                return Err("该历史文件的成图已被清理，请重新选择图片来源。".to_string());
            }
            (
                path_buf.clone(),
                origin.clone().unwrap_or_else(|| path.clone()),
                title.clone().unwrap_or_else(|| "历史图片".to_string()),
                "local".to_string(),
            )
        }
    };

    // 2) 渲染成屏幕尺寸的满屏图（保证没有黑边）。
    let render_target = paths
        .library
        .join(format!("wallpaper-{stamp}.jpg"));
    let render_src = source_path.clone();
    let render_out = render_target.clone();
    let render_fit = fit.clone();
    let (screen_w, screen_h) = (screen.width, screen.height);
    tauri::async_runtime::spawn_blocking(move || {
        imaging::render_to_screen(
            &render_src,
            &render_out,
            screen_w,
            screen_h,
            &render_fit,
            quality,
        )
    })
    .await
    .map_err(|e| format!("图像渲染任务失败：{e}"))??;

    // 3) 导出到用户选择的保存目录。
    let exported_path = {
        let export_dir = PathBuf::from(&settings.download_dir);
        let export = export_dir.join(format!("Wallpaper-Studio-{stamp}.jpg"));
        match std::fs::create_dir_all(&export_dir)
            .and_then(|_| std::fs::copy(&render_target, &export))
        {
            Ok(_) => Some(export.display().to_string()),
            Err(err) => {
                log::warn!("导出成图到保存目录失败: {err}");
                None
            }
        }
    };

    // 4) 应用。
    let mut wallpaper_step: Option<StepResult> = None;
    let mut lockscreen_step: Option<StepResult> = None;
    let mut needs_admin = false;

    if request.wallpaper {
        wallpaper_step = Some(match wallpaper::set_wallpaper(&render_target) {
            Ok(path) => StepResult {
                success: true,
                message: format!("桌面壁纸已更新（{} × {}）", screen.width, screen.height),
                hint: Some(path.display().to_string()),
            },
            Err(err) => StepResult {
                success: false,
                message: format!("桌面壁纸设置失败：{err}"),
                hint: None,
            },
        });
    }

    if request.lockscreen {
        let outcome = lockscreen::apply(&paths, &render_target);
        needs_admin = outcome.needs_admin;
        if let Some(installed) = outcome.installed.as_ref() {
            log::info!("锁屏图像文件：{}", installed.display());
        }
        lockscreen_step = Some(outcome.step);
    }

    // 5) 缩略图与历史。
    let thumb_path = paths.thumbs.join(format!("{stamp}.jpg"));
    let thumb_src = render_target.clone();
    let thumb_out = thumb_path.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || {
        imaging::write_thumbnail(&thumb_src, &thumb_out, 640, 76)
    })
    .await;

    let mut history_entry = HistoryEntry {
        id: stamp.clone(),
        title,
        source: source_kind,
        origin,
        wallpaper_path: render_target.display().to_string(),
        thumb_path: Some(thumb_path.display().to_string()),
        applied_at: chrono::Utc::now().timestamp_millis(),
        fit: fit.clone(),
        resolution: format!("{} × {}", screen.width, screen.height),
        lock_target: request.lockscreen,
        thumb: None,
    };

    if thumb_path.exists() {
        if let Ok((data_url, _, _)) = imaging::preview_data_url(&thumb_path, 420, 72) {
            history_entry.thumb = Some(data_url);
        }
    }

    {
        let history_paths = paths.clone();
        let state = app.state::<AppState>();
        let mut guard = state.history.lock().map_err(|e| e.to_string())?;
        guard.retain(|item| item.id != history_entry.id);
        guard.insert(0, history_entry.clone());
        if settings.keep_library_copy {
            guard.truncate(80);
        } else {
            guard.truncate(10);
        }
        let _ = config::save_history(&history_paths, &guard);
    }

    if settings.auto_prune {
        if settings.keep_library_copy {
            let prune_paths = paths.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                services::library::prune_all(&prune_paths)
            })
            .await;
        } else {
            let prune_paths = paths.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                services::library::prune_dir(&prune_paths.library, 3);
                services::library::prune_dir(&prune_paths.thumbs, 10);
            })
            .await;
        }
    }

    let ok = wallpaper_step.as_ref().map(|s| s.success).unwrap_or(true)
        && lockscreen_step.as_ref().map(|s| s.success).unwrap_or(true);

    let message = if ok {
        if request.wallpaper && request.lockscreen {
            "壁纸与锁屏已同时更新".to_string()
        } else if request.wallpaper {
            "壁纸已更新".to_string()
        } else {
            "锁屏已更新".to_string()
        }
    } else {
        "部分设置未能生效，请查看提示".to_string()
    };

    Ok(ApplyReport {
        ok,
        needs_admin,
        wallpaper: wallpaper_step,
        lockscreen: lockscreen_step,
        rendered_path: Some(render_target.display().to_string()),
        exported_path,
        message,
        history: Some(history_entry),
    })
}

#[tauri::command]
pub async fn refresh_history(app: AppHandle) -> Result<Vec<HistoryEntry>, String> {
    let (paths, history) = {
        let state = app.state::<AppState>();
        let paths = state.paths.clone();
        let history = state.history.lock().map_err(|e| e.to_string())?.clone();
        (paths, history)
    };
    tauri::async_runtime::spawn_blocking(move || {
        services::library::history_with_thumbs(&paths, &history)
    })
    .await
    .map_err(|e| format!("刷新历史失败：{e}"))
}

#[tauri::command]
pub async fn remove_history(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut history = state.history.lock().map_err(|e| e.to_string())?;
    history.retain(|item| item.id != id);
    config::save_history(&state.paths, &history)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    let state = app.state::<AppState>();
    let clean = settings.sanitized();
    if clean.download_dir.trim().is_empty() {
        return Err("请选择图片保存目录。".to_string());
    }
    if let Err(err) = std::fs::create_dir_all(&clean.download_dir) {
        return Err(format!("无法创建保存目录：{err}"));
    }
    config::save_settings(&state.paths, &clean)?;
    *state.settings.lock().map_err(|e| e.to_string())? = clean.clone();
    Ok(clean)
}

#[tauri::command]
pub fn screen_info() -> ScreenInfo {
    sysinfo::screen_info()
}

#[tauri::command]
pub fn reveal_path(path: String) -> Result<(), String> {
    let target = PathBuf::from(&path);
    let mut command = std::process::Command::new("explorer.exe");
    if target.is_dir() {
        command.arg(&target);
    } else if target.exists() {
        command.arg(format!("/select,{}", target.display()));
    } else {
        let parent = target.parent().unwrap_or(Path::new("."));
        command.arg(parent);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("无法打开资源管理器：{e}"))
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("仅允许打开 HTTPS 链接。".to_string());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", &url])
            .creation_flags(0x0800_0000)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("打开链接失败：{e}"))
    }
    #[cfg(not(windows))]
    {
        Err("仅支持 Windows。".to_string())
    }
}

#[tauri::command]
pub fn clear_cache(app: AppHandle) -> Result<u64, String> {
    let paths = paths_of(&app)?;
    let freed = config::dir_size(&paths.cache);
    if paths.cache.exists() {
        std::fs::remove_dir_all(&paths.cache).map_err(|e| format!("清理缓存失败：{e}"))?;
    }
    std::fs::create_dir_all(&paths.cache).map_err(|e| format!("重建缓存目录失败：{e}"))?;
    Ok(freed)
}

#[tauri::command]
pub fn reset_wallpaper() -> Result<StepResult, String> {
    match wallpaper::restore_default() {
        Ok(path) => Ok(StepResult {
            success: true,
            message: "已恢复 Windows 默认壁纸。".to_string(),
            hint: Some(path.display().to_string()),
        }),
        Err(err) => Ok(StepResult {
            success: false,
            message: err,
            hint: None,
        }),
    }
}

#[tauri::command]
pub fn reset_lockscreen() -> StepResult {
    lockscreen::reset()
}

#[tauri::command]
pub fn relaunch_admin(app: AppHandle) -> Result<(), String> {
    sysinfo::relaunch_elevated()?;
    app.exit(0);
    Ok(())
}
