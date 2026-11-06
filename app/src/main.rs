//! Sennheiser Control — unofficial Windows tray app for the Sennheiser MOMENTUM 4.
//!
//! Flags: `--minimized` (start in the tray, like the "Start minimized" setting), `--sim` (simulated headset), `--debug` (verbose log),
//! `--quit` (ask the running instance to exit), `--render-icons DIR` (dev: icon previews + app.ico).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod crash_report;
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

/// Warnings, errors and panics also go here; created on first use so it only exists when something went wrong.
pub const ERROR_LOG: &str = "sennheiser-control-error.log";

struct FileLogger {
    file: parking_lot::Mutex<std::fs::File>,
    errors: parking_lot::Mutex<Option<std::fs::File>>,
    error_path: std::path::PathBuf,
    level: log::LevelFilter,
}

impl log::Log for FileLogger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= self.level
            && (m.target().starts_with("engine")
                || m.target().starts_with("bluetooth")
                || m.target().starts_with("headset")
                || m.target().starts_with("app")
                || m.target().starts_with("sennheiser_control")
                || m.level() <= log::Level::Warn)
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            let line = format!("{t:.3} {:<5} {}: {}", r.level(), r.target(), r.args());
            let _ = writeln!(self.file.lock(), "{line}");
            if r.level() <= log::Level::Warn {
                let mut errors = self.errors.lock();
                if errors.is_none() {
                    *errors = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&self.error_path)
                        .ok();
                }
                if let Some(f) = errors.as_mut() {
                    let _ = writeln!(f, "{line}");
                }
            }
            // Sentry: errors become events, warnings become breadcrumbs.
            // Panics are captured by Sentry's panic integration; the panic
            // hook's own log line is skipped inside `capture_error_log`.
            match r.level() {
                log::Level::Error => crate::crash_report::capture_error_log(&line),
                log::Level::Warn => {
                    crate::crash_report::breadcrumb(&line, sentry::Level::Warning);
                }
                _ => {}
            }
        }
    }
    fn flush(&self) {
        let _ = self.file.lock().flush();
        if let Some(f) = self.errors.lock().as_mut() {
            let _ = f.flush();
        }
    }
}

/// Keep a log below `max` bytes by moving the previous run's content aside.
fn rotate(path: &std::path::Path, old: &str, max: u64) {
    if std::fs::metadata(path).is_ok_and(|m| m.len() > max) {
        let _ = std::fs::rename(path, path.with_file_name(old));
    }
}

fn init_logging(debug: bool) {
    let dir = settings::dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("app.log");
    rotate(&path, "app.old.log", 2_000_000);
    let error_path = dir.join(ERROR_LOG);
    rotate(&error_path, "sennheiser-control-error.old.log", 1_000_000);
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
            errors: parking_lot::Mutex::new(None),
            error_path,
            level,
        }))
        .is_ok()
        {
            log::set_max_level(level);
        }
    }
    // the release build has no console, so a panic would otherwise vanish without a trace
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        log::error!(
            target: "app",
            "crash in thread '{}' ({} {}): {info}\n{}",
            thread.name().unwrap_or("unnamed"),
            APP_NAME,
            env!("CARGO_PKG_VERSION"),
            std::backtrace::Backtrace::force_capture()
        );
        log::logger().flush();
        default_hook(info);
    }));
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    let simulate = has("--sim");
    if let Some(i) = args.iter().position(|a| a == "--render-icons") {
        let dir = std::path::PathBuf::from(args.get(i + 1).map(String::as_str).unwrap_or("."));
        tray::icon::dev_render(&dir);
        return Ok(());
    }
    // before logging creates the new folder; the simulator never touches the real install
    if !simulate {
        platform::migrate_legacy_install();
    }
    // the simulator keeps its own instance and settings file, separate from the real app
    settings::set_simulated(simulate);
    // Load settings early: the crash-reporting opt-out must be known before Sentry starts.
    let settings = settings::Settings::load();
    // Init Sentry before `init_logging` so the panic hook below chains onto
    // Sentry's panic handler. Keep the guard alive for the process lifetime.
    let _sentry_guard =
        crash_report::init(settings.crash_reporting, has("--debug"), simulate);
    init_logging(has("--debug"));

    let (show_event, quit_event) = match platform::claim_single_instance(has("--quit"), simulate) {
        platform::Instance::Secondary => return Ok(()),
        platform::Instance::Primary {
            show_event,
            quit_event,
        } => (show_event, quit_event),
    };
    let minimized = has("--minimized") || settings.start_minimized;
    log::info!(
        "starting {APP_NAME} {} (minimized={minimized}, sim={simulate})",
        env!("CARGO_PKG_VERSION")
    );
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
    .inspect_err(|e| {
        log::error!("window failed: {e}");
        log::logger().flush();
    })
}
