//! Observable application state.

use protocol::eq::BANDS;
use protocol::{
    AntiWind, AudioMode, BtMode, Capabilities, ChargingState, Codec, PersonalizedSoundMode,
    PhysicalState, PromptMode, Update, Version,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Smart Control Plus "NoiseControl" variant 2 modes for the M4 (`products/m4` config: adaptive, custom, off).
pub enum NoiseMode {
    /// ANC on, adaptive ANC (mode pair 3 = 1); the headset manages the level itself.
    Adaptive,
    /// ANC on, adaptive off; the user picks the noise-cancelling ⟷ transparency balance (`ANC_Transparency`).
    Custom,
    /// `ANC_Status = 0` (fw ≥ 2.13.32).
    Off,
}

impl NoiseMode {
    pub fn label(self) -> &'static str {
        match self {
            NoiseMode::Adaptive => "Adaptive",
            NoiseMode::Custom => "Custom",
            NoiseMode::Off => "Off",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct EqState {
    pub bands: [f32; BANDS],
    pub bass_boost: bool,
    pub min_db: f32,
    pub max_db: f32,
    /// Band values have been read from the headset.
    pub known: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PairedHost {
    pub index: u8,
    pub name: String,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct HeadsetState {
    pub firmware: Option<Version>,
    pub model_id: Option<String>,
    pub serial: Option<String>,
    pub hw_revision: Option<u16>,
    pub sample_rate: Option<u32>,
    pub battery: Option<u8>,
    pub charging: Option<ChargingState>,
    pub codec: Option<Codec>,
    pub anti_wind: Option<AntiWind>,
    pub adaptive: Option<bool>,
    pub anc_status: Option<bool>,
    pub transparency: Option<u8>,
    pub sidetone: Option<u8>,
    pub audio_mode: Option<AudioMode>,
    /// Audio mode before podcast mode was switched on, restored when it is switched off (as Smart Control Plus does).
    pub pre_podcast_mode: Option<AudioMode>,
    pub prompt_mode: Option<PromptMode>,
    pub on_head_detection: Option<bool>,
    pub smart_pause: Option<bool>,
    pub auto_call: Option<bool>,
    pub comfort_call: Option<bool>,
    pub touch_lock: Option<bool>,
    pub bt_mode: Option<BtMode>,
    pub physical_state: Option<PhysicalState>,
    pub auto_off_seconds: Option<u16>,
    pub personalized_sound: Option<PersonalizedSoundMode>,
    pub eq: EqState,
    pub paired_hosts: Vec<PairedHost>,
    pub paired_count: Option<u16>,
    /// Paired-list index of the host we're talking through (this PC).
    pub paired_own_index: Option<u8>,
    pub paired_max_connections: Option<u8>,
    pub caps: Capabilities,
    /// Gen1 bookkeeping (level shift ×10 and bass flag) needed to decode band gains.
    pub eq1_level_shift_x10: Option<i8>,
}

pub const SIDETONE_MAX: u8 = 4;

impl HeadsetState {
    pub fn charging_now(&self) -> bool {
        self.charging == Some(ChargingState::Charging)
    }

    /// Current noise mode. `ANC_Status = off` wins; otherwise the adaptive flag decides (the headset ignores
    /// transparency while adaptive is on — verified on fw 3.38.3).
    pub fn noise_mode(&self) -> Option<NoiseMode> {
        if self.caps.anc_off && self.anc_status == Some(false) {
            return Some(NoiseMode::Off);
        }
        self.adaptive.map(|a| {
            if a {
                NoiseMode::Adaptive
            } else {
                NoiseMode::Custom
            }
        })
    }

    pub fn variant(&self) -> Option<&str> {
        self.model_id.as_deref().and_then(protocol::m4_variant)
    }

    /// Smart Control Plus "Hi-Res 24 bit/96 kHz" (= `BluetoothCompatibilityMode` better audio).
    pub fn hi_res(&self) -> Option<bool> {
        self.bt_mode.map(|m| m == BtMode::BestAudio)
    }

    /// A hearing profile exists on the headset, so personalization can be switched on and off.
    pub fn personalization_ready(&self) -> bool {
        matches!(
            self.personalized_sound,
            Some(
                PersonalizedSoundMode::Disabled
                    | PersonalizedSoundMode::Standby
                    | PersonalizedSoundMode::Enabled
            )
        )
    }

    /// The user EQ is what the headset is playing through (audio mode `UserEq`).
    pub fn eq_active(&self) -> bool {
        self.audio_mode == Some(AudioMode::UserEq)
    }

    pub fn podcast(&self) -> bool {
        self.audio_mode.is_some_and(AudioMode::is_podcast)
    }

    /// Apply a decoded update; returns whether anything changed.
    pub fn apply(&mut self, u: &Update) -> bool {
        let before = self.clone();
        match u {
            Update::FirmwareVersion(v) => {
                self.firmware = Some(*v);
                self.caps = Capabilities::for_m4(*v);
                if self.eq.max_db == 0.0 {
                    self.eq.min_db = protocol::eq::MIN_GAIN_DB;
                    self.eq.max_db = protocol::eq::MAX_GAIN_DB;
                }
            }
            Update::ModelId(m) => self.model_id = Some(m.clone()),
            Update::SerialNumber(s) => self.serial = Some(s.clone()),
            Update::HwRevision(h) => self.hw_revision = Some(*h),
            Update::SampleRate(r) => self.sample_rate = Some(*r),
            Update::Battery(b) => self.battery = *b,
            Update::Charging(c) => self.charging = Some(*c),
            Update::Codec(c) => self.codec = *c,
            Update::AntiWind(a) => self.anti_wind = Some(*a),
            Update::Adaptive(a) => self.adaptive = Some(*a),
            Update::AncStatus(s) => self.anc_status = Some(*s),
            Update::Transparency(t) => self.transparency = Some(*t),
            Update::Sidetone(s) => self.sidetone = Some(*s),
            Update::AudioMode(m) => {
                if m.is_podcast() && !self.podcast() {
                    self.pre_podcast_mode = self.audio_mode;
                }
                self.audio_mode = Some(*m);
            }
            Update::PromptMode(m) => self.prompt_mode = Some(*m),
            Update::OnHeadDetection(v) => self.on_head_detection = Some(*v),
            Update::SmartPause(v) => self.smart_pause = Some(*v),
            Update::AutoCall(v) => self.auto_call = Some(*v),
            Update::ComfortCall(v) => self.comfort_call = Some(*v),
            Update::TouchLock(v) => self.touch_lock = Some(*v),
            Update::BtMode(m) => self.bt_mode = Some(*m),
            Update::PhysicalState(p) => self.physical_state = Some(*p),
            Update::Timer { id: 0, seconds } => self.auto_off_seconds = Some(*seconds),
            Update::PersonalizedSound(m) => self.personalized_sound = *m,
            Update::Eq2Config {
                min_x10, max_x10, ..
            } => {
                self.eq.min_db = *min_x10 as f32 / 10.0;
                self.eq.max_db = *max_x10 as f32 / 10.0;
            }
            Update::Eq2Slider { band, gain_x10 } => {
                if let Some(b) = self.eq.bands.get_mut(*band as usize) {
                    *b = protocol::eq::gen2_db(*gain_x10);
                }
                if *band as usize == BANDS - 1 {
                    self.eq.known = true;
                }
            }
            Update::Eq2Sliders(g) => {
                for (i, v) in g.iter().take(BANDS).enumerate() {
                    self.eq.bands[i] = protocol::eq::gen2_db(*v);
                }
                self.eq.known = g.len() >= BANDS;
            }
            Update::Eq2SubMode(m) => self.eq.bass_boost = *m == 1,
            Update::Eq1LevelShift(v) => self.eq1_level_shift_x10 = Some(*v),
            Update::Eq1Knob {
                index: protocol::eq::KNOB_Y_INDEX,
                value,
            } => {
                if let Some(bb) = protocol::eq::gen1_knob_bass_boost(*value) {
                    self.eq.bass_boost = bb;
                }
            }
            Update::Eq1Params(p) if p.first() == Some(&5) => {
                if let Some(g) = protocol::eq::gen1_decode(
                    &protocol::eq::M4_GEN1,
                    self.eq1_level_shift_x10.unwrap_or(0),
                    self.eq.bass_boost,
                    p,
                ) {
                    self.eq.bands = g;
                    self.eq.known = true;
                }
            }
            Update::PairedCount(n) => {
                self.paired_count = Some(*n);
                self.paired_hosts.retain(|h| u16::from(h.index) < *n);
            }
            Update::PairedInfo {
                index,
                status,
                name,
                ..
            } => {
                let host = PairedHost {
                    index: *index,
                    name: name.clone(),
                    connected: *status == 1,
                };
                match self.paired_hosts.iter_mut().find(|h| h.index == *index) {
                    Some(h) => *h = host,
                    None => {
                        self.paired_hosts.push(host);
                        self.paired_hosts.sort_by_key(|h| h.index);
                    }
                }
            }
            Update::PairedOwnIndex(i) => self.paired_own_index = Some(*i),
            Update::PairedMaxConnections(n) => self.paired_max_connections = Some(*n),
            Update::PairedConnection { index, status } => {
                if let Some(h) = self.paired_hosts.iter_mut().find(|h| h.index == *index) {
                    h.connected = *status == 1;
                }
            }
            _ => {}
        }
        *self != before
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LinkState {
    /// No supported headset connected to Windows.
    #[default]
    Idle,
    Connecting,
    Connected,
    /// Last attempt failed; retrying.
    Retrying(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceEntry {
    pub id: String,
    pub name: String,
    pub address: String,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppState {
    /// Supported (MOMENTUM 4) headsets paired with Windows.
    pub devices: Vec<DeviceEntry>,
    pub active: Option<String>,
    pub link: LinkState,
    pub headset: Option<HeadsetState>,
    pub discovery_done: bool,
    pub last_error: Option<String>,
    pub simulated: bool,
}

impl AppState {
    pub fn active_device(&self) -> Option<&DeviceEntry> {
        let id = self.active.as_ref()?;
        self.devices.iter().find(|d| &d.id == id)
    }

    pub fn connected_devices(&self) -> impl Iterator<Item = &DeviceEntry> {
        self.devices.iter().filter(|d| d.connected)
    }

    /// Device name, with an address suffix when several paired headsets share the same name.
    pub fn display_name(&self, d: &DeviceEntry) -> String {
        let dup = self.devices.iter().filter(|x| x.name == d.name).count() > 1;
        if dup && d.address.len() >= 5 {
            format!(
                "{} · {}",
                d.name,
                d.address[d.address.len() - 5..].to_uppercase()
            )
        } else {
            d.name.clone()
        }
    }

    /// Headset state, only when the control link is up.
    pub fn live(&self) -> Option<&HeadsetState> {
        (self.link == LinkState::Connected)
            .then_some(self.headset.as_ref())
            .flatten()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    SelectDevice(String),
    SetNoiseMode(NoiseMode),
    /// Custom-mode balance: 0 = full noise cancelling, 50 = neutral, 100 = full transparency (`ANC_Transparency`).
    SetTransparency(u8),
    SetAntiWind(AntiWind),
    SetSidetone(u8),
    SetEqBands([f32; BANDS]),
    SetBassBoost(bool),
    /// `false` = Neutral (audio mode `Off`, the headset's own tuning); `true` = play through the user EQ.
    SetEqEnabled(bool),
    SetPodcast(bool),
    SetOnHeadDetection(bool),
    SetSmartPause(bool),
    SetAutoCall(bool),
    SetComfortCall(bool),
    SetTouchLock(bool),
    /// Hi-Res 24 bit/96 kHz on (better audio) or off (better compatibility).
    SetHiRes(bool),
    SetPromptMode(PromptMode),
    SetAutoOff(u16),
    SetPersonalizedSound(bool),
    ConnectHost(u8),
    DisconnectHost(u8),
    /// Confirmed by the user in the UI. Removes a disconnected host (never this PC) from the pairing list.
    ForgetHost(u8),
    /// Confirmed by the user in the UI. Clears settings and all pairings on the headset.
    FactoryReset,
    Refresh,
    Reconnect,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_tracks_changes_and_caps() {
        let mut s = HeadsetState::default();
        assert!(s.apply(&Update::FirmwareVersion(Version::new(3, 38, 3))));
        assert_eq!(s.caps.eq, protocol::EqGen::Gen2);
        assert!(s.apply(&Update::Transparency(0)));
        assert!(!s.apply(&Update::Transparency(0)));
        assert_eq!(s.noise_mode(), None);
        s.apply(&Update::Adaptive(true));
        assert_eq!(s.noise_mode(), Some(NoiseMode::Adaptive));
        s.apply(&Update::Adaptive(false));
        assert_eq!(s.noise_mode(), Some(NoiseMode::Custom));
        // fw 3.38 has the ANC master switch: ANC_Status off means Off
        s.apply(&Update::AncStatus(false));
        assert_eq!(s.noise_mode(), Some(NoiseMode::Off));
        s.apply(&Update::Eq2Sliders(vec![30, 0, -15, 0, 60]));
        assert_eq!(s.eq.bands, [3.0, 0.0, -1.5, 0.0, 6.0]);
        assert!(s.eq.known);
        s.apply(&Update::ModelId("M4AEBT Denim".into()));
        assert_eq!(s.variant(), Some("Denim"));
    }
}
