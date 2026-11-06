//! Linux helpers: autostart via .desktop file, lockfile single instance, theme via gsettings.

use std::path::PathBuf;

// ----------------------------------------------------------------------------------------------- autostart

const DESKTOP_FILE: &str = "sennheiser-control.desktop";

fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(std::env::temp_dir)
}

fn desktop_path() -> PathBuf {
    config_dir().join("autostart").join(DESKTOP_FILE)
}

pub fn autostart_enabled() -> bool {
    desktop_path().is_file()
}

pub fn set_autostart(enable: bool) -> std::io::Result<()> {
    let p = desktop_path();
    if enable {
        let exe = std::env::current_exe()?;
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            &p,
            format!(
                "[Desktop Entry]\nType=Application\nName=Sennheiser Control\nExec=\"{}\"\nX-GNOME-Autostart-enabled=true\n",
                exe.display()
            ),
        )
    } else {
        match std::fs::remove_file(&p) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Ok(()) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

/// No registry or rename to migrate on Linux.
pub fn migrate_legacy_install() {}

// ----------------------------------------------------------------------------------------------- theme

fn gsettings(args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("gsettings")
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn prefers_light() -> Option<bool> {
    if let Some(s) = gsettings(&["get", "org.gnome.desktop.interface", "color-scheme"]) {
        if s.contains("prefer-light") {
            return Some(true);
        }
        if s.contains("prefer-dark") {
            return Some(false);
        }
    }
    // older GNOME: infer from the gtk-theme name.
    let s = gsettings(&["get", "org.gnome.desktop.interface", "gtk-theme"])?;
    Some(!s.to_lowercase().contains("dark"))
}

/// The tray loop asks every couple of seconds; don't spawn gsettings that often.
fn cached_light() -> bool {
    static CELL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *CELL.get_or_init(|| prefers_light().unwrap_or(false))
}

/// Tray glyph theme.
pub fn taskbar_is_light() -> bool {
    cached_light()
}

/// App theme when the user picked "System".
pub fn apps_are_light() -> bool {
    cached_light()
}

// ----------------------------------------------------------------------------------------------- single instance

pub enum Instance {
    /// First instance. Both events are 0 on Linux (no cross-process signalling yet).
    Primary {
        show_event: usize,
        quit_event: usize,
    },
    /// Another instance is running (or `--quit` found one); just exit.
    Secondary,
}

fn lock_path(sim: bool) -> PathBuf {
    let mut d = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    d.push(if sim {
        "sennheiser-control-sim.lock"
    } else {
        "sennheiser-control.lock"
    });
    d
}

// ponytail: /proc liveness check instead of a file-lock dep; stale locks self-heal.
fn pid_alive(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/cmdline")).is_ok()
}

/// `_quit` is accepted but best-effort: a running instance is left alone (quit via the tray menu).
pub fn claim_single_instance(_quit: bool, sim: bool) -> Instance {
    let p = lock_path(sim);
    if let Ok(prev) = std::fs::read_to_string(&p)
        && let Ok(pid) = prev.trim().parse::<u32>()
        && pid_alive(pid)
    {
        return Instance::Secondary;
    }
    let _ = std::fs::write(&p, std::process::id().to_string());
    Instance::Primary {
        show_event: 0,
        quit_event: 0,
    }
}

// ----------------------------------------------------------------------------------------------- wake event

/// Channel-based wake-up for the tray loop (replaces the Win32 event handle).
#[derive(Clone)]
pub struct WakeEvent {
    tx: crossbeam_channel::Sender<()>,
    rx: crossbeam_channel::Receiver<()>,
}

impl WakeEvent {
    pub fn new() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        WakeEvent { tx, rx }
    }
    pub fn signal(&self) {
        let _ = self.tx.send(());
    }
    /// Drain pending wakes, then block up to `timeout` for the next one.
    pub fn wait_timeout(&self, timeout: std::time::Duration) {
        while self.rx.try_recv().is_ok() {}
        let _ = self.rx.recv_timeout(timeout);
    }
}

// ----------------------------------------------------------------------------------------------- shell & window

pub fn open_bluetooth_settings() {
    // ponytail: settings:// URIs only work on GNOME; try each desktop's panel in turn.
    const CANDIDATES: &[(&str, &[&str])] = &[
        ("kcmshell6", &["kcm_bluetooth"]),        // KDE
        ("gnome-control-center", &["bluetooth"]), // GNOME
        ("blueman-manager", &[]),                 // Blueman (MATE/Xfce/…)
        ("blueberry", &[]),                       // Cinnamon
    ];
    for (bin, args) in CANDIDATES {
        match std::process::Command::new(bin).args(*args).spawn() {
            Ok(_) => return,
            Err(e) => log::debug!("{bin} not available: {e}"),
        }
    }
    let _ = std::process::Command::new("xdg-open")
        .arg("settings://bluetooth")
        .spawn();
}

/// Tray-flyout positioning is Windows-only; the window opens at the default spot.
pub fn work_area() -> Option<(i32, i32, i32, i32)> {
    None
}

/// DWM rounded corners are Windows-only.
pub fn round_corners(_hwnd: usize) {}

/// User-visible OS name for labels.
pub fn os_name() -> &'static str {
    "Linux"
}

/// Autostart toggle label.
pub fn autostart_label() -> &'static str {
    "Start at login"
}

#[cfg(test)]
mod tests {
    /// Touches the real ~/.config/autostart entry (restores the previous state). Run explicitly: `cargo test -p app -- --ignored`.
    #[test]
    #[ignore]
    fn autostart_roundtrip() {
        let before = super::autostart_enabled();
        super::set_autostart(true).unwrap();
        assert!(super::autostart_enabled());
        let body = std::fs::read_to_string(super::desktop_path()).unwrap();
        assert!(body.contains("Exec=\""), "{body}");
        super::set_autostart(false).unwrap();
        assert!(!super::autostart_enabled());
        if before {
            super::set_autostart(true).unwrap();
        }
    }
}
