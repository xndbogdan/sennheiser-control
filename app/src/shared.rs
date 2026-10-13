//! State shared between the UI thread and the tray thread.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crossbeam_channel::Sender;
use engine::{Action, Engine};
use parking_lot::Mutex;

use crate::platform::WakeEvent;
use crate::settings::Settings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiCmd {
    Show,
    Toggle,
    OpenSettings,
    Quit,
}

pub struct Shared {
    pub engine: Engine,
    settings: Mutex<Settings>,
    settings_version: AtomicU64,
    pub ui_tx: Sender<UiCmd>,
    pub egui_ctx: egui::Context,
    pub tray_wake: WakeEvent,
    pub quitting: AtomicBool,
}

impl Shared {
    pub fn new(
        engine: Engine,
        settings: Settings,
        ui_tx: Sender<UiCmd>,
        egui_ctx: egui::Context,
        tray_wake: WakeEvent,
    ) -> Self {
        Self {
            engine,
            settings: Mutex::new(settings),
            settings_version: AtomicU64::new(0),
            ui_tx,
            egui_ctx,
            tray_wake,
            quitting: AtomicBool::new(false),
        }
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().clone()
    }

    pub fn settings_version(&self) -> u64 {
        self.settings_version.load(Ordering::SeqCst)
    }

    /// Mutate, persist and broadcast settings.
    pub fn update_settings(&self, f: impl FnOnce(&mut Settings)) {
        let new = {
            let mut s = self.settings.lock();
            let before = s.clone();
            f(&mut s);
            if *s == before {
                return;
            }
            s.clone()
        };
        new.save();
        self.settings_version.fetch_add(1, Ordering::SeqCst);
        self.wake_all();
    }

    pub fn send_ui(&self, cmd: UiCmd) {
        let _ = self.ui_tx.send(cmd);
        self.egui_ctx.request_repaint();
    }

    pub fn wake_all(&self) {
        self.tray_wake.signal();
        self.egui_ctx.request_repaint();
    }

    pub fn dispatch(&self, a: Action) {
        self.engine.dispatch(a);
    }
}

/// Wording and order shared by the window and the tray menu.
pub const WIND_ORDER: [protocol::AntiWind; 3] = [
    protocol::AntiWind::Off,
    protocol::AntiWind::Auto,
    protocol::AntiWind::Max,
];
pub const SIDETONE_LABELS: [&str; 5] = ["Off", "Low", "Medium", "High", "Max"];
pub const PROMPTS_TITLE: &str = "Tones and voice prompts";
pub const SIDETONE_TITLE: &str = "Sidetone";
pub const COMFORT_CALLS_TITLE: &str = "Comfort calls";

/// Name for a Custom-mode balance. Smart Control Plus treats `ANC_Transparency` as one scale with a neutral
/// centre: below 50 is noise cancelling of strength (50 − level) × 2, above 50 transparency of (level − 50) × 2.
pub fn custom_level_label(level: u8) -> String {
    let level = level.min(100);
    match level {
        0 => "Max noise cancelling".into(),
        50 => "Neutral".into(),
        100 => "Max transparency".into(),
        l if l < 50 => format!("Noise cancelling {}%", (50 - l) * 2),
        l => format!("Transparency {}%", (l - 50) * 2),
    }
}

#[cfg(test)]
mod tests {
    use super::custom_level_label;

    #[test]
    fn custom_labels_split_at_neutral() {
        assert_eq!(custom_level_label(0), "Max noise cancelling");
        assert_eq!(custom_level_label(25), "Noise cancelling 50%");
        assert_eq!(custom_level_label(49), "Noise cancelling 2%");
        assert_eq!(custom_level_label(50), "Neutral");
        assert_eq!(custom_level_label(51), "Transparency 2%");
        assert_eq!(custom_level_label(75), "Transparency 50%");
        assert_eq!(custom_level_label(100), "Max transparency");
    }
}
