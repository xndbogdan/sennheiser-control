//! Sennheiser Control — unofficial Windows tray app for the Sennheiser MOMENTUM 4.
//!
//! Flags: `--minimized` (start in the tray), `--sim` (simulated headset), `--debug` (verbose log),
//! `--quit` (ask the running instance to exit), `--render-icons DIR` (dev: icon previews + app.ico).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod platform;
mod settings;
mod shared;
mod tray;
mod ui;

use std::io::Write;
use std::sync::Arc;

use engine::{Engine, EngineOptions};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub const APP_NAME: &str = "Sennheiser Control";
/// Folder / registry / mutex name.
pub const APP_DIR: &str = "SennheiserControl";
/// Name used before the rename; its settings folder and autostart entry are migrated once.
pub const LEGACY_APP_DIR: &str = "HeadsetControl";

struct FileLogger {
    file: parking_lot::Mutex<std::fs::File>,
    level: log::LevelFilter,
}

impl log::Log for FileLogger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= self.level
            && (m.target().starts_with("engine")
                || m.target().starts_with("bluetooth")
                || m.target().starts_with("headset")
                || m.target().starts_with("app")
                || m.level() <= log::Level::Warn)
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            let _ = writeln!(
                self.file.lock(),
                "{t:.3} {:<5} {}: {}",
                r.level(),
                r.target(),
                r.args()
            );
        }
    }
    fn flush(&self) {
        let _ = self.file.lock().flush();
    }
}

fn init_logging(debug: bool) {
    let dir = settings::dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("app.log");
    if let Ok(meta) = std::fs::metadata(&path)
        && meta.len() > 2_000_000
    {
        let _ = std::fs::rename(&path, dir.join("app.old.log"));
    }
    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let level = if debug {
            log::LevelFilter::Trace
        } else {
            log::LevelFilter::Info
        };
        if log::set_boxed_logger(Box::new(FileLogger {
            file: parking_lot::Mutex::new(file),
            level,
        }))
        .is_ok()
        {
            log::set_max_level(level);
        }
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    let (minimized, simulate) = (has("--minimized"), has("--sim"));
    if let Some(i) = args.iter().position(|a| a == "--render-icons") {
        let dir = std::path::PathBuf::from(args.get(i + 1).map(String::as_str).unwrap_or("."));
        tray::icon::dev_render(&dir);
        return Ok(());
    }
    // before logging creates the new folder; the simulator never touches the real install
    if !simulate {
        platform::migrate_legacy_install();
    }
    init_logging(has("--debug"));

    // the simulator keeps its own instance and settings file, separate from the real app
    settings::set_simulated(simulate);
    let (show_event, quit_event) = match platform::claim_single_instance(has("--quit"), simulate) {
        platform::Instance::Secondary => return Ok(()),
        platform::Instance::Primary {
            show_event,
            quit_event,
        } => (show_event, quit_event),
    };
    log::info!(
        "starting {APP_NAME} {} (minimized={minimized}, sim={simulate})",
        env!("CARGO_PKG_VERSION")
    );

    let settings = settings::Settings::load();
    let icon = egui::IconData {
        rgba: tray::icon::app_icon(64),
        width: 64,
        height: 64,
    };
    // dev aid for screenshots: HC_DEV_HEIGHT=1500 opens a taller window
    let height = std::env::var("HC_DEV_HEIGHT")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(720.0);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_app_id(APP_DIR)
            .with_inner_size([408.0, height])
            .with_min_inner_size([380.0, 480.0])
            .with_decorations(false)
            .with_resizable(true)
            .with_visible(!minimized)
            .with_icon(Arc::new(icon)),
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| {
            ui::theme::install_fonts(&cc.egui_ctx);
            if let Ok(h) = cc.window_handle()
                && let RawWindowHandle::Win32(w) = h.as_raw()
            {
                platform::round_corners(w.hwnd.get() as usize);
            }
            let tray_wake = platform::WakeEvent::new();
            let ctx = cc.egui_ctx.clone();
            let engine = Engine::start(
                EngineOptions {
                    simulate,
                    preferred_device: settings.preferred_device.clone(),
                },
                move || {
                    tray_wake.signal();
                    ctx.request_repaint();
                },
            );
            let (ui_tx, ui_rx) = crossbeam_channel::unbounded();
            let shared = Arc::new(shared::Shared::new(
                engine,
                settings,
                ui_tx,
                cc.egui_ctx.clone(),
                tray_wake,
            ));
            let tray_thread = tray::spawn(shared.clone());
            if show_event != 0 && quit_event != 0 {
                let shared = shared.clone();
                std::thread::Builder::new()
                    .name("instance".into())
                    .spawn(move || {
                        use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
                        use windows::Win32::System::Threading::{INFINITE, WaitForMultipleObjects};
                        let handles = [HANDLE(show_event as *mut _), HANDLE(quit_event as *mut _)];
                        loop {
                            let r = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) };
                            if r == WAIT_OBJECT_0 {
                                shared.send_ui(shared::UiCmd::Show);
                            } else if r.0 == WAIT_OBJECT_0.0 + 1 {
                                shared.send_ui(shared::UiCmd::Quit);
                                return;
                            } else {
                                return;
                            }
                        }
                    })
                    .ok();
            }
            let mut app = ui::HeadsetApp::new(shared, ui_rx, tray_thread, !minimized);
            // dev aids for screenshots: HC_DEV_PAGE picks a page or opens a panel, HC_DEV_SCROLL=px scrolls
            match std::env::var("HC_DEV_PAGE").as_deref() {
                Ok("settings") => app.page = ui::Page::Settings,
                Ok("devices") => app.show_devices = true,
                Ok("panels") => app.open_dev_panels(),
                _ => {}
            }
            app.dev_scroll = std::env::var("HC_DEV_SCROLL")
                .ok()
                .and_then(|v| v.parse().ok());
            Ok(Box::new(app))
        }),
    )
}
