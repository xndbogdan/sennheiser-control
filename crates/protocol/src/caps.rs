//! Feature availability derived from the firmware version (`m4.json` → parameters, PROTOCOL.md §5).

use crate::types::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EqGen {
    /// Legacy 3-band knob EQ (fw < 2.12.0) — not supported by this app.
    #[default]
    Unsupported,
    Gen1,
    Gen2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capabilities {
    pub rfcomm_control: bool,
    pub on_head_detection: bool,
    pub smart_pause: bool,
    pub auto_power_off: bool,
    pub voice_prompts: bool,
    pub touch_lock: bool,
    /// The touch lock survives a reboot (`keepsMmiLockedAfterReboot`, fw ≥ 2.13.32).
    pub touch_lock_persists: bool,
    /// Smart Control Plus "Hi-Res 24 bit/96 kHz" switch (`AudioModes`, aptX 96 kHz). On the wire this is the old
    /// app's `BluetoothCompatibilityMode`: 0 = Hi-Res (better audio), 1 = better compatibility.
    pub hi_res_audio: bool,
    pub physical_state: bool,
    pub sound_personalization: bool,
    pub auto_answer_calls: bool,
    pub comfort_call: bool,
    pub sidetone: bool,
    pub eq: EqGen,
    /// Hidden by the official app (min fw 99.99.99).
    pub low_latency: bool,
    /// `ANC_Status` is the master ANC on/off switch ("ANC off" added by fw 3.37.x / Smart Control Plus).
    pub anc_off: bool,
}

pub const RFCOMM_MIN: Version = Version::new(2, 11, 26);
pub const ON_HEAD_MIN: Version = Version::new(2, 6, 3);
pub const VOICE_PROMPT_MIN: Version = Version::new(2, 6, 12);
pub const FEATURES_2_12: Version = Version::new(2, 12, 0);
pub const AUTO_ANSWER_MIN: Version = Version::new(2, 13, 28);
/// Smart Control Plus `products/m4` config: `AudioModes` (Hi-Res) and `SoundPersonalization` from 2.13.18.
pub const SCP_2_13_18: Version = Version::new(2, 13, 18);
pub const EQ_GEN2_MIN: Version = Version::new(3, 18, 0);
/// Smart Control Plus `products/m4` config adds the "off" noise-control mode at 2.13.32.
pub const ANC_OFF_MIN: Version = Version::new(2, 13, 32);

impl Capabilities {
    pub fn for_m4(fw: Version) -> Self {
        let at = |v: Version| fw >= v;
        Self {
            rfcomm_control: at(RFCOMM_MIN),
            on_head_detection: at(ON_HEAD_MIN),
            smart_pause: at(ON_HEAD_MIN),
            auto_power_off: at(ON_HEAD_MIN),
            voice_prompts: at(VOICE_PROMPT_MIN),
            touch_lock: at(FEATURES_2_12),
            touch_lock_persists: at(ANC_OFF_MIN),
            hi_res_audio: at(SCP_2_13_18),
            physical_state: at(FEATURES_2_12),
            sound_personalization: at(SCP_2_13_18),
            auto_answer_calls: at(AUTO_ANSWER_MIN),
            comfort_call: true,
            sidetone: true,
            eq: if at(EQ_GEN2_MIN) {
                EqGen::Gen2
            } else if at(FEATURES_2_12) {
                EqGen::Gen1
            } else {
                EqGen::Unsupported
            },
            low_latency: false,
            anc_off: at(ANC_OFF_MIN),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gates() {
        let c = Capabilities::for_m4(Version::new(3, 38, 3));
        assert_eq!(c.eq, EqGen::Gen2);
        assert!(c.auto_answer_calls && c.touch_lock && c.rfcomm_control);
        let old = Capabilities::for_m4(Version::new(2, 12, 5));
        assert_eq!(old.eq, EqGen::Gen1);
        assert!(!old.auto_answer_calls);
        assert_eq!(
            Capabilities::for_m4(Version::new(2, 11, 0)).eq,
            EqGen::Unsupported
        );
        assert!(!Capabilities::for_m4(Version::new(2, 11, 0)).rfcomm_control);
    }
}
