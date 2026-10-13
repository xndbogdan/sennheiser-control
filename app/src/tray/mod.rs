//! Tray thread: owns the notification-area icon and its menu, runs its own Win32 message loop.

pub mod icon;
pub mod menu;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;

use engine::{AppState, LinkState};
use tray_icon::menu::MenuEvent;
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetSystemMetricsForDpi};
use windows::Win32::UI::Shell::{
    NIF_INFO, NIIF_USER, NIIF_WARNING, NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, MsgWaitForMultipleObjects, PM_REMOVE, PeekMessageW, QS_ALLINPUT,
    SM_CXSMICON, TranslateMessage, WM_QUIT,
};

use crate::shared::{Shared, UiCmd};
use icon::{IconRenderer, TrayVisual};

pub fn spawn(shared: Arc<Shared>) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("tray".into())
        .spawn(move || run(shared))
        .expect("spawn tray thread")
}

fn visual(state: &AppState) -> TrayVisual {
    if let Some(h) = state.live() {
        return TrayVisual::Battery {
            percent: h.battery,
            charging: h.charging_now(),
        };
    }
    let connected = state.active_device().is_some_and(|d| d.connected);
    match &state.link {
        LinkState::Connecting | LinkState::Retrying(_) if connected => TrayVisual::Connecting,
        _ => TrayVisual::Disconnected,
    }
}

fn tray_icon_size() -> u32 {
    unsafe { GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem()).max(16) as u32 }
}

struct Rendered {
    visual: TrayVisual,
    size: u32,
    light: bool,
}

struct LowBattery {
    /// Level for which we already warned (re-arms when charging or above threshold).
    warned: bool,
}

fn run(shared: Arc<Shared>) {
    let renderer = IconRenderer::new();
    let state = shared.engine.snapshot();
    let settings = shared.settings();
    let mut rendered = Rendered {
        visual: visual(&state),
        size: tray_icon_size(),
        light: crate::platform::taskbar_is_light(),
    };
    let make_icon = |r: &Rendered| {
        Icon::from_rgba(renderer.tray(r.visual, r.size, r.light), r.size, r.size).ok()
    };

    let tray = match TrayIconBuilder::new()
        .with_tooltip(menu::tooltip(&state))
        .with_icon(make_icon(&rendered).expect("tray icon"))
        .with_menu(Box::new(menu::build(
            &state,
            &settings,
            crate::platform::autostart_enabled(),
        )))
        .with_menu_on_left_click(false)
        .build()
    {
        Ok(t) => t,
        Err(e) => {
            log::error!("tray icon failed: {e}");
            return;
        }
    };

    {
        let shared = shared.clone();
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = e
            {
                shared.send_ui(UiCmd::Toggle);
            }
        }));
    }
    {
        let shared = shared.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| menu::handle(&e.id.0, &shared)));
    }

    let mut seen = (
        shared.engine.version(),
        shared.settings_version(),
        crate::platform::autostart_enabled(),
    );
    let mut low = LowBattery { warned: false };
    loop {
        unsafe {
            let handles = [shared.tray_wake.handle()];
            MsgWaitForMultipleObjects(Some(&handles), false, 2000, QS_ALLINPUT);
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    return;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if shared.quitting.load(Ordering::SeqCst) {
            break;
        }
        let now = (
            shared.engine.version(),
            shared.settings_version(),
            crate::platform::autostart_enabled(),
        );
        let state = shared.engine.snapshot();
        let next = Rendered {
            visual: visual(&state),
            size: tray_icon_size(),
            light: crate::platform::taskbar_is_light(),
        };
        if next.visual != rendered.visual
            || next.size != rendered.size
            || next.light != rendered.light
        {
            rendered = next;
            let _ = tray.set_icon(make_icon(&rendered));
        }
        if now != seen {
            seen = now;
            let settings = shared.settings();
            let _ = tray.set_tooltip(Some(menu::tooltip(&state)));
            tray.set_menu(Some(Box::new(menu::build(&state, &settings, now.2))));
            check_low_battery(&tray, &state, settings.low_battery_threshold, &mut low);
        }
    }
    drop(tray);
}

fn check_low_battery(tray: &TrayIcon, state: &AppState, threshold: u8, low: &mut LowBattery) {
    let Some(h) = state.live() else { return };
    let Some(p) = h.battery else { return };
    if threshold == 0 || h.charging_now() || p > threshold {
        low.warned = false;
        return;
    }
    if low.warned {
        return;
    }
    low.warned = true;
    let name = state
        .active_device()
        .map(|d| d.name.as_str())
        .unwrap_or("Headset");
    notify(
        tray,
        &format!("{name} battery low"),
        &format!("{p}% remaining — time to charge."),
        true,
    );
}

/// Show a Windows notification attached to our tray icon (shown as a toast on Windows 10/11).
/// tray-icon does not expose the icon's `uID`; its window hosts only our icon, so probe the few small ids.
pub fn notify(tray: &TrayIcon, title: &str, body: &str, warning: bool) {
    static UID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: windows::Win32::Foundation::HWND(tray.window_handle() as *mut _),
        uFlags: NIF_INFO,
        dwInfoFlags: if warning { NIIF_WARNING } else { NIIF_USER },
        ..Default::default()
    };
    copy_wide(&mut nid.szInfoTitle, title);
    copy_wide(&mut nid.szInfo, body);
    let known = UID.load(Ordering::Relaxed);
    let candidates: Vec<u32> = if known != 0 {
        vec![known]
    } else {
        (1..=16).collect()
    };
    for uid in candidates {
        nid.uID = uid;
        if unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) }.as_bool() {
            UID.store(uid, Ordering::Relaxed);
            log::info!("notification '{title}' shown (uID {uid})");
            return;
        }
    }
    log::warn!("notification '{title}' could not be shown");
}

fn copy_wide(dst: &mut [u16], s: &str) {
    let n = dst.len() - 1;
    for (d, c) in dst.iter_mut().zip(s.encode_utf16().take(n)) {
        *d = c;
    }
}
