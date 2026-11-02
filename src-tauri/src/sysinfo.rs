//! Windows 系统信息、权限检测与提权。

use crate::models::ScreenInfo;

#[cfg(windows)]
mod win {
    use super::ScreenInfo;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS,
    };
    use windows_sys::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CMONITORS, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    };

    fn wide(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
    }

    pub fn screen_info() -> ScreenInfo {
        let mut devmode: DEVMODEW = unsafe { std::mem::zeroed() };
        devmode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        let ok = unsafe { EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, &mut devmode) };

        let (width, height) = if ok != 0 && devmode.dmPelsWidth > 0 {
            (devmode.dmPelsWidth, devmode.dmPelsHeight)
        } else {
            unsafe {
                (
                    GetSystemMetrics(windows_sys::Win32::UI::WindowsAndMessaging::SM_CXSCREEN) as u32,
                    GetSystemMetrics(windows_sys::Win32::UI::WindowsAndMessaging::SM_CYSCREEN) as u32,
                )
            }
        };

        let (virtual_width, virtual_height, monitors) = unsafe {
            (
                GetSystemMetrics(SM_CXVIRTUALSCREEN).max(width as i32) as u32,
                GetSystemMetrics(SM_CYVIRTUALSCREEN).max(height as i32) as u32,
                GetSystemMetrics(SM_CMONITORS).max(1) as u32,
            )
        };

        ScreenInfo {
            width,
            height,
            virtual_width,
            virtual_height,
            monitors,
            is_windows: true,
        }
    }

    pub fn is_admin() -> bool {
        unsafe { IsUserAnAdmin() != 0 }
    }

    pub fn relaunch_elevated() -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| format!("无法定位程序路径：{e}"))?;
        let verb = wide(OsStr::new("runas"));
        let file = wide(exe.as_os_str());
        let params = wide(OsStr::new(""));
        let dir = wide(exe.parent().unwrap_or(exe.as_path()).as_os_str());

        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                file.as_ptr(),
                params.as_ptr(),
                dir.as_ptr(),
                1,
            )
        };

        if result as isize > 32 {
            Ok(())
        } else {
            Err("提权请求被取消或失败，请以管理员身份手动运行程序。".to_string())
        }
    }

    pub fn os_caption() -> String {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let Ok(key) = hklm.open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion") else {
            return "Windows".to_string();
        };
        let product: String = key.get_value("ProductName").unwrap_or_default();
        let build: String = key.get_value("CurrentBuild").unwrap_or_default();
        let display: String = key.get_value("DisplayVersion").unwrap_or_default();
        if product.is_empty() {
            "Windows".to_string()
        } else if display.is_empty() {
            format!("{product} (Build {build})")
        } else {
            format!("{product} {display} (Build {build})")
        }
    }
}

pub fn screen_info() -> ScreenInfo {
    #[cfg(windows)]
    {
        win::screen_info()
    }
    #[cfg(not(windows))]
    {
        ScreenInfo {
            width: 1920,
            height: 1080,
            virtual_width: 1920,
            virtual_height: 1080,
            monitors: 1,
            is_windows: false,
        }
    }
}

pub fn is_admin() -> bool {
    #[cfg(windows)]
    {
        win::is_admin()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub fn relaunch_elevated() -> Result<(), String> {
    #[cfg(windows)]
    {
        win::relaunch_elevated()
    }
    #[cfg(not(windows))]
    {
        Err("仅支持 Windows。".to_string())
    }
}

pub fn os_caption() -> String {
    #[cfg(windows)]
    {
        win::os_caption()
    }
    #[cfg(not(windows))]
    {
        "Unknown".to_string()
    }
}
