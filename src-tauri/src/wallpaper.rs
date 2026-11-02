//! 桌面壁纸设置（Windows 原生）。

use std::path::{Path, PathBuf};

use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
use winreg::RegKey;

const DESKTOP_KEY: &str = r"Control Panel\Desktop";

fn write_desktop_registry(image: &Path) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let desktop = hkcu
        .open_subkey_with_flags(DESKTOP_KEY, KEY_SET_VALUE)
        .map_err(|e| format!("无法写入桌面设置注册表：{e}"))?;

    let value = image.display().to_string();
    desktop
        .set_value("Wallpaper", &value)
        .map_err(|e| format!("写入 Wallpaper 失败：{e}"))?;
    // 成图已经按屏幕分辨率渲染，固定使用「填充」即可保证满屏。
    desktop
        .set_value("WallpaperStyle", &"10")
        .map_err(|e| format!("写入 WallpaperStyle 失败：{e}"))?;
    desktop
        .set_value("TileWallpaper", &"0")
        .map_err(|e| format!("写入 TileWallpaper 失败：{e}"))?;
    Ok(())
}

#[cfg(windows)]
pub fn set_wallpaper(image: &Path) -> Result<PathBuf, String> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_SETDESKWALLPAPER,
    };

    let absolute = std::path::absolute(image).unwrap_or_else(|_| image.to_path_buf());
    if !absolute.exists() {
        return Err("壁纸文件不存在。".to_string());
    }

    write_desktop_registry(&absolute)?;

    let wide: Vec<u16> = absolute
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let ok = unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            wide.as_ptr() as *mut std::ffi::c_void,
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
    };

    if ok == 0 {
        return Err(format!(
            "系统拒绝更新壁纸（错误码 {}）",
            std::io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or_default()
        ));
    }

    // 通知资源管理器立即重绘桌面（不依赖异步刷新）。
    let _ = std::process::Command::new("rundll32.exe")
        .args(["user32.dll,UpdatePerUserSystemParameters", "1", "True"])
        .creation_flags(0x0800_0000)
        .spawn();

    Ok(absolute)
}

#[cfg(not(windows))]
pub fn set_wallpaper(_image: &Path) -> Result<PathBuf, String> {
    Err("仅支持 Windows 系统。".to_string())
}

#[cfg(windows)]
pub fn restore_default() -> Result<PathBuf, String> {
    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    let candidates = [
        PathBuf::from(&system_root).join(r"Web\Wallpaper\Windows\img0.jpg"),
        PathBuf::from(&system_root).join(r"Web\Wallpaper\Windows\img19.jpg"),
        PathBuf::from(&system_root).join(r"Web\4K\Wallpaper\Windows\img0_3840x2160.jpg"),
    ];
    for candidate in candidates {
        if candidate.exists() {
            return set_wallpaper(&candidate);
        }
    }
    Err("未找到 Windows 默认壁纸文件。".to_string())
}

#[cfg(not(windows))]
pub fn restore_default() -> Result<PathBuf, String> {
    Err("仅支持 Windows 系统。".to_string())
}
