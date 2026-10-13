//! Persisted app preferences (`%APPDATA%\SennheiserControl\settings.json`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::APP_DIR;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemePref {
    #[default]
    System,
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomPreset {
    pub name: String,
    pub bands: [f32; protocol::eq::BANDS],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub preferred_device: Option<String>,
    /// Closing the window hides it to the tray instead of quitting.
    pub close_to_tray: bool,
    /// Show a notification when the battery drops to this level (0 = off).
    pub low_battery_threshold: u8,
    pub theme: ThemePref,
    pub custom_presets: Vec<CustomPreset>,
    /// The "Custom" EQ curve: the last curve that matched no preset.
    pub custom_eq: Option<[f32; protocol::eq::BANDS]>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            preferred_device: None,
            close_to_tray: true,
            low_battery_threshold: 15,
            theme: ThemePref::System,
            custom_presets: Vec::new(),
            custom_eq: None,
        }
    }
}

pub fn dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join(APP_DIR)
}

static SIMULATED: AtomicBool = AtomicBool::new(false);

/// `--sim` runs read and write `settings-sim.json`, so the simulated headset never becomes the preferred device.
pub fn set_simulated(sim: bool) {
    SIMULATED.store(sim, Ordering::Relaxed);
}

fn path() -> PathBuf {
    dir().join(if SIMULATED.load(Ordering::Relaxed) {
        "settings-sim.json"
    } else {
        "settings.json"
    })
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let p = path();
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                let tmp = p.with_extension("json.tmp");
                if std::fs::write(&tmp, json).is_ok() {
                    let _ = std::fs::rename(&tmp, &p);
                }
            }
            Err(e) => log::warn!("saving settings: {e}"),
        }
    }
}
