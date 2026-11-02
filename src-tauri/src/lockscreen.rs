//! 锁屏图像设置。
//!
//! Windows 并未提供公开的锁屏设置 API，实际可行的方式是：
//! 1. 把「按屏幕分辨率渲染好的满屏图」复制到系统目录；
//! 2. 写入 `PersonalizationCSP`（优先 HKLM，其次 HKCU）；
//! 3. 关闭 Spotlight / 轮播，刷新 LockApp。
//!
//! 步骤 2 需要管理员权限，因此这里会给出明确的降级提示。

use std::path::{Path, PathBuf};
use std::process::Command;

use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_SET_VALUE};
use winreg::RegKey;

use crate::config::Paths;
use crate::models::StepResult;

const CSP_PATH: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\PersonalizationCSP";
const CDM_PATH: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\ContentDeliveryManager";

pub struct LockOutcome {
    pub step: StepResult,
    pub needs_admin: bool,
    pub installed: Option<PathBuf>,
}

#[cfg(windows)]
fn silent(command: &str, args: &[&str]) {
    use std::os::windows::process::CommandExt;
    let _ = Command::new(command)
        .args(args)
        .creation_flags(0x0800_0000)
        .spawn();
}

#[cfg(not(windows))]
fn silent(_command: &str, _args: &[&str]) {}

fn file_url(path: &Path) -> String {
    let text = path.display().to_string().replace('\\', "/");
    if text.starts_with("//") {
        format!("file:{text}")
    } else {
        format!("file:///{text}")
    }
}

fn write_csp(root: RegKey, image: &Path) -> Result<(), String> {
    let key = match root.open_subkey_with_flags(CSP_PATH, KEY_SET_VALUE | KEY_QUERY_VALUE) {
        Ok(key) => key,
        Err(_) => root
            .create_subkey(CSP_PATH)
            .map_err(|e| format!("无法创建注册表项：{e}"))?
            .0,
    };

    key.set_value("LockScreenImagePath", &image.display().to_string())
        .map_err(|e| format!("写入 LockScreenImagePath 失败：{e}"))?;
    key.set_value("LockScreenImageUrl", &file_url(image))
        .map_err(|e| format!("写入 LockScreenImageUrl 失败：{e}"))?;
    key.set_value("LockScreenImageStatus", &1u32)
        .map_err(|e| format!("写入 LockScreenImageStatus 失败：{e}"))?;
    Ok(())
}

fn disable_spotlight() {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey_with_flags(CDM_PATH, KEY_SET_VALUE) {
        let _ = key.set_value("RotatingLockScreenEnabled", &0u32);
        let _ = key.set_value("RotatingLockScreenOverlayEnabled", &0u32);
        let _ = key.set_value("SubscribedContent-338387Enabled", &0u32);
    }
}

fn refresh_lockscreen() {
    silent("rundll32.exe", &["user32.dll,UpdatePerUserSystemParameters", "1", "True"]);
    // LockApp 会在下次锁屏时自动重建。
    silent("taskkill.exe", &["/F", "/IM", "LockApp.exe"]);
}

/// 复制到系统锁屏目录；失败时回退到应用数据目录。
fn install_image(paths: &Paths, rendered: &Path) -> (PathBuf, bool) {
    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    let system_dir = PathBuf::from(system_root).join("Web").join("Screen");
    let target = system_dir.join("wellmax-lockscreen.jpg");

    if std::fs::create_dir_all(&system_dir).is_ok() && std::fs::copy(rendered, &target).is_ok() {
        return (target, true);
    }

    let fallback = paths.library.join("lockscreen-current.jpg");
    if let Err(e) = std::fs::copy(rendered, &fallback) {
        log::warn!("锁屏图像回退复制失败: {e}");
    }
    (fallback, false)
}

pub fn apply(paths: &Paths, rendered: &Path) -> LockOutcome {
    let (installed, in_system_dir) = install_image(paths, rendered);
    disable_spotlight();

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let machine_result = write_csp(hklm, &installed);
    let user_result = if machine_result.is_ok() {
        Ok(())
    } else {
        write_csp(hkcu, &installed)
    };

    refresh_lockscreen();

    match (machine_result, user_result) {
        (Ok(()), _) => LockOutcome {
            step: StepResult {
                success: true,
                message: format!("锁屏图像已写入 {}", installed.display()),
                hint: Some("按 Win + L 即可预览满屏锁屏效果。".to_string()),
            },
            needs_admin: false,
            installed: Some(installed),
        },
        (Err(machine_err), Ok(())) => LockOutcome {
            step: StepResult {
                success: true,
                message: "已写入当前用户锁屏配置。".to_string(),
                hint: Some(format!(
                    "部分系统仅在管理员模式下才会让锁屏生效（HKLM: {machine_err}）。建议以管理员身份重启。"
                )),
            },
            needs_admin: true,
            installed: Some(installed),
        },
        (Err(machine_err), Err(user_err)) => LockOutcome {
            step: StepResult {
                success: false,
                message: format!("锁屏设置未生效：{user_err}"),
                hint: Some(if in_system_dir {
                    format!("系统目录写入成功，但注册表被拒绝（{machine_err}）。请以管理员身份重新运行。")
                } else {
                    "需要管理员权限才能写入锁屏目录与系统注册表。".to_string()
                }),
            },
            needs_admin: true,
            installed: Some(installed),
        },
    }
}

pub fn reset() -> StepResult {
    let mut removed = 0usize;
    for hive in [RegKey::predef(HKEY_LOCAL_MACHINE), RegKey::predef(HKEY_CURRENT_USER)] {
        if let Ok(key) = hive.open_subkey_with_flags(CSP_PATH, KEY_SET_VALUE) {
            for name in ["LockScreenImagePath", "LockScreenImageUrl", "LockScreenImageStatus"] {
                if key.delete_value(name).is_ok() {
                    removed += 1;
                }
            }
        }
    }
    refresh_lockscreen();

    if removed > 0 {
        StepResult {
            success: true,
            message: "已移除自定义锁屏配置，系统将恢复默认锁屏。".to_string(),
            hint: Some("若仍显示旧图片，锁屏一次后即可刷新。".to_string()),
        }
    } else {
        StepResult {
            success: false,
            message: "未找到可移除的自定义锁屏配置（可能需要管理员权限）。".to_string(),
            hint: Some("请以管理员身份重试。".to_string()),
        }
    }
}
