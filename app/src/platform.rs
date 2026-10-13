//! Small Windows helpers: autostart, single instance, theme, work area, window chrome.

use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, RECT,
};
use windows::Win32::Graphics::Dwm::{
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, OpenEventW, SetEvent,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETWORKAREA, SW_SHOWNORMAL, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};
use windows::core::{HSTRING, PCWSTR, w};
use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const PERSONALIZE_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";

// ----------------------------------------------------------------------------------------------- autostart

pub fn autostart_enabled() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY, KEY_READ)
        .and_then(|k| k.get_value::<String, _>(crate::APP_DIR))
        .is_ok()
}

pub fn set_autostart(enable: bool) -> std::io::Result<()> {
    let key =
        RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_READ | KEY_WRITE)?;
    if enable {
        let exe = std::env::current_exe()?;
        key.set_value(
            crate::APP_DIR,
            &format!("\"{}\" --minimized", exe.display()),
        )
    } else {
        match key.delete_value(crate::APP_DIR) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            r => r,
        }
    }
}

/// One-time move from the pre-rename "HeadsetControl" install: settings folder and Start-with-Windows entry.
pub fn migrate_legacy_install() {
    let new_dir = crate::settings::dir();
    let old_dir = new_dir.with_file_name(crate::LEGACY_APP_DIR);
    if old_dir.is_dir() && !new_dir.exists() {
        let _ = std::fs::rename(&old_dir, &new_dir);
    }
    let Ok(key) =
        RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_READ | KEY_WRITE)
    else {
        return;
    };
    if key.get_value::<String, _>(crate::LEGACY_APP_DIR).is_ok()
        && key.delete_value(crate::LEGACY_APP_DIR).is_ok()
    {
        // the old entry points at the old exe name; register this one instead
        let _ = set_autostart(true);
    }
}

// ----------------------------------------------------------------------------------------------- theme

fn personalize_dword(name: &str) -> Option<u32> {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(PERSONALIZE_KEY, KEY_READ)
        .ok()?
        .get_value::<u32, _>(name)
        .ok()
}

/// Taskbar / notification area uses the light theme.
pub fn taskbar_is_light() -> bool {
    personalize_dword("SystemUsesLightTheme") == Some(1)
}

/// Apps use the light theme.
pub fn apps_are_light() -> bool {
    personalize_dword("AppsUseLightTheme").unwrap_or(1) == 1
}

// ----------------------------------------------------------------------------------------------- single instance

/// (instance mutex, show event, quit event). `--sim` runs get their own set, so they never hand off to (or
/// get mistaken for) the real instance.
const NAMES: [PCWSTR; 3] = [
    w!("Local\\SennheiserControl.Instance"),
    w!("Local\\SennheiserControl.Show"),
    w!("Local\\SennheiserControl.Quit"),
];
const SIM_NAMES: [PCWSTR; 3] = [
    w!("Local\\SennheiserControl.Sim.Instance"),
    w!("Local\\SennheiserControl.Sim.Show"),
    w!("Local\\SennheiserControl.Sim.Quit"),
];

pub enum Instance {
    /// We are the first instance. The events are signalled when another instance asks us to show / quit.
    Primary {
        show_event: usize,
        quit_event: usize,
    },
    /// Another instance is running and has been signalled (or `--quit` found nothing to quit).
    Secondary,
}

/// `quit = true` asks a running instance to exit instead of showing its window.
pub fn claim_single_instance(quit: bool, sim: bool) -> Instance {
    let [mutex_name, show_name, quit_name] = if sim { SIM_NAMES } else { NAMES };
    unsafe {
        let mutex = CreateMutexW(None, true, mutex_name);
        if mutex.is_ok() && GetLastError() == ERROR_ALREADY_EXISTS {
            let name = if quit { quit_name } else { show_name };
            if let Ok(ev) = OpenEventW(EVENT_MODIFY_STATE, false, name) {
                let _ = SetEvent(ev);
                let _ = CloseHandle(ev);
            }
            return Instance::Secondary;
        }
        if quit {
            return Instance::Secondary;
        }
        // keep the mutex handle open for the lifetime of the process
        std::mem::forget(mutex);
        let ev = |name| {
            CreateEventW(None, false, false, name)
                .map(|h| h.0 as usize)
                .unwrap_or(0)
        };
        Instance::Primary {
            show_event: ev(show_name),
            quit_event: ev(quit_name),
        }
    }
}

/// Auto-reset event used to wake a waiting thread (e.g. the tray loop) from any thread.
#[derive(Clone, Copy)]
pub struct WakeEvent(pub usize);

impl WakeEvent {
    pub fn new() -> Self {
        unsafe {
            WakeEvent(
                CreateEventW(None, false, false, PCWSTR::null())
                    .map(|h| h.0 as usize)
                    .unwrap_or(0),
            )
        }
    }
    pub fn handle(self) -> HANDLE {
        HANDLE(self.0 as *mut _)
    }
    pub fn signal(self) {
        if self.0 != 0 {
            unsafe {
                let _ = SetEvent(self.handle());
            }
        }
    }
}

// ----------------------------------------------------------------------------------------------- shell & window

pub fn open_bluetooth_settings() {
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &HSTRING::from("ms-settings:bluetooth"),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

/// Primary monitor work area in physical pixels (left, top, right, bottom).
pub fn work_area() -> Option<(i32, i32, i32, i32)> {
    let mut r = RECT::default();
    unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut r as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .ok()?;
    }
    Some((r.left, r.top, r.right, r.bottom))
}

/// Ask DWM for Windows 11 rounded corners on our borderless window.
pub fn round_corners(hwnd: usize) {
    if hwnd == 0 {
        return;
    }
    unsafe {
        let pref = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            HWND(hwnd as *mut _),
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const _,
            std::mem::size_of_val(&pref) as u32,
        );
    }
}

#[cfg(test)]
mod tests {
    /// Touches the real HKCU Run key (restores the previous state). Run explicitly: `cargo test -p app -- --ignored`.
    #[test]
    #[ignore]
    fn autostart_roundtrip() {
        let before = super::autostart_enabled();
        super::set_autostart(true).unwrap();
        assert!(super::autostart_enabled());
        let cmd: String = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey(super::RUN_KEY)
            .unwrap()
            .get_value(crate::APP_DIR)
            .unwrap();
        assert!(
            cmd.ends_with("\" --minimized") && cmd.starts_with('"'),
            "{cmd}"
        );
        super::set_autostart(false).unwrap();
        assert!(!super::autostart_enabled());
        super::set_autostart(before).unwrap();
    }
}
