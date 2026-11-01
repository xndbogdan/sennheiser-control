//! Feature availability per product and firmware version: the device model `parameters` and the official app's
//! `<Product>.createProductFeatures` (PROTOCOL.md §5 and §8). Only the M4 has been checked on hardware.

use crate::product::Product;
use crate::types::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EqGen {
    /// Legacy 3-band knob EQ — not supported by this app.
    #[default]
    Unsupported,
    Gen1,
    Gen2,
}

/// How noise control works, following each product's tile in the official app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NoiseStyle {
    #[default]
    None,
    /// Adaptive ANC on/off (`ANC` pair `(3, x)`) plus the `ANC_Transparency` balance for the non-adaptive
    /// "Custom" mode (MOMENTUM 4, ACCENTUM Plus). Anti-wind is a separate setting.
    AdaptiveBalance,
    /// Noise cancelling is always on; transparent hearing goes on or off with `TransparentHearing_Status`
    /// (ACCENTUM, `Ac1Product` bundle.pretty.js:3709). Anti-wind is a separate setting.
    AncOrTransparency,
    /// Off / ANC / ANC outdoor (anti-wind) through `ANC_Status` and the `ANC` pair `(1, x)`; transparent hearing is a
    /// separate switch (ACCENTUM True Wireless, MOMENTUM True Wireless 3 and 4, e.g. `Tw4Product` :10134).
    AncTile,
    /// ANC / ANC outdoor / Transparency, with no "all off" (MOMENTUM Sport, `MomentumSport1Product` :7483).
    SportTile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capabilities {
    pub rfcomm_control: bool,
    pub noise: NoiseStyle,
    /// Wind noise reduction as its own Off / Max / Auto setting (`ANC` pair `(1, x)`).
    pub anti_wind: bool,
    /// A separate transparent hearing on/off switch (`TransparentHearing_Status`).
    pub transparency_switch: bool,
    pub on_head_detection: bool,
    /// With on-head detection it only works while that is on; without it, it is a plain switch.
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
    /// Highest sidetone step (`phoneCallsMaxValue`); 0 = no sidetone control.
    pub sidetone_max: u8,
    pub eq: EqGen,
    /// Audio mode 0 turns the EQ off ("Neutral"). The `audio-mode-1` feature only knows 1 = user EQ, 2 = speech,
    /// 3 = flat, so older firmware has no EQ-off mode; the M4 was verified on hardware.
    pub eq_off: bool,
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

const ANY: Version = Version::new(0, 0, 0);
const NEVER: Version = Version::new(99, 99, 99);

/// Gen2 comes with the `user-eq-2` feature, Gen1 needs `Eq5Band_Gen1_MinFwVersion` (`MultiBandEqFactory`,
/// bundle.pretty.js:12525).
fn eq_gen(fw: Version, gen1: Version, gen2: Version) -> EqGen {
    if fw >= gen2 {
        EqGen::Gen2
    } else if fw >= gen1 {
        EqGen::Gen1
    } else {
        EqGen::Unsupported
    }
}

impl Capabilities {
    pub fn new(product: Product, fw: Version) -> Self {
        let at = |v: Version| fw >= v;
        // what every product besides the M4 shares: voice prompts, auto power-off and the EQ/audio-mode features
        let base = |gen1: Version, gen2: Version| Self {
            rfcomm_control: true,
            auto_power_off: true,
            voice_prompts: true,
            eq: eq_gen(fw, gen1, gen2),
            eq_off: at(gen2),
            ..Self::default()
        };
        // the `features` lists and `parameters` of each device model; gates noted where the app applies one
        match product {
            Product::Momentum4 => Self::for_m4(fw),
            // Acp1Product (:3784): the M4 set without Hi-Res; touch lock `customizableControlsMinFwVersion` 3.14.6;
            // user-eq-2 / audio-mode-2 / sound-personalization-2 from 3.16.4
            Product::AccentumPlus => Self {
                noise: NoiseStyle::AdaptiveBalance,
                anti_wind: true,
                on_head_detection: at(ON_HEAD_MIN),
                smart_pause: at(ON_HEAD_MIN),
                auto_power_off: at(ON_HEAD_MIN),
                voice_prompts: at(VOICE_PROMPT_MIN),
                touch_lock: at(Version::new(3, 14, 6)),
                physical_state: at(FEATURES_2_12),
                // sound-personalization-2 (fw ≥ 3.16.4) uses a different state enum; not mapped yet
                sound_personalization: at(FEATURES_2_12) && !at(Version::new(3, 16, 4)),
                auto_answer_calls: at(ON_HEAD_MIN) && at(AUTO_ANSWER_MIN),
                comfort_call: true,
                sidetone_max: 4,
                ..base(FEATURES_2_12, Version::new(3, 16, 4))
            },
            // Ac1Product (:3543): sidetone 0–2, no wear sensor, touch lock or comfort call
            Product::Accentum => Self {
                noise: NoiseStyle::AncOrTransparency,
                anti_wind: true,
                sidetone_max: 2,
                ..base(FEATURES_2_12, Version::new(3, 16, 4))
            },
            // ATW1Product (:4098)
            Product::AccentumTw => Self {
                noise: NoiseStyle::AncTile,
                transparency_switch: true,
                smart_pause: true,
                auto_answer_calls: true,
                touch_lock: true,
                sidetone_max: 2,
                ..base(Version::new(2, 10, 5), Version::new(5, 0, 26))
            },
            // Tw3Product (:9422): ANC from `ancMinFwVersion` 2.3.0, no sidetone, no user-eq-2
            Product::MomentumTw3 => Self {
                rfcomm_control: at(RFCOMM_MIN),
                noise: if at(Version::new(2, 3, 0)) {
                    NoiseStyle::AncTile
                } else {
                    NoiseStyle::None
                },
                transparency_switch: true,
                smart_pause: true,
                auto_answer_calls: true,
                touch_lock: true,
                hi_res_audio: at(Version::new(2, 9, 0)),
                sound_personalization: at(FEATURES_2_12),
                ..base(Version::new(2, 10, 5), NEVER)
            },
            // Tw4Product (:9789): sidetone 0–3 from `Sidetone_minFwVersion` 5.3.0
            Product::MomentumTw4 => Self {
                noise: NoiseStyle::AncTile,
                transparency_switch: true,
                smart_pause: true,
                auto_answer_calls: true,
                touch_lock: true,
                hi_res_audio: true,
                sound_personalization: !at(Version::new(5, 0, 0)),
                sidetone_max: if at(Version::new(5, 3, 0)) { 3 } else { 0 },
                ..base(ANY, Version::new(5, 0, 0))
            },
            // MomentumSport1Product (:7094): its touch lock is a different property (`Touch_Locked`), left out
            Product::MomentumSport => Self {
                noise: NoiseStyle::SportTile,
                smart_pause: true,
                auto_answer_calls: true,
                hi_res_audio: at(Version::new(3, 17, 14)),
                sidetone_max: 2,
                ..base(ANY, Version::new(3, 16, 4))
            },
        }
    }

    pub fn for_m4(fw: Version) -> Self {
        let at = |v: Version| fw >= v;
        Self {
            rfcomm_control: at(RFCOMM_MIN),
            noise: NoiseStyle::AdaptiveBalance,
            anti_wind: true,
            transparency_switch: false,
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
            sidetone_max: 4,
            eq: eq_gen(fw, FEATURES_2_12, EQ_GEN2_MIN),
            eq_off: true,
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
        assert_eq!(
            Capabilities::new(Product::Momentum4, Version::new(3, 38, 3)),
            c
        );
    }

    #[test]
    fn other_products() {
        // the firmware from the GitHub issue
        let acp = Capabilities::new(Product::AccentumPlus, Version::new(3, 36, 1));
        assert_eq!(
            (acp.eq, acp.noise),
            (EqGen::Gen2, NoiseStyle::AdaptiveBalance)
        );
        assert!(acp.touch_lock && acp.auto_answer_calls && acp.eq_off);
        assert!(!acp.hi_res_audio && !acp.anc_off && !acp.sound_personalization);
        let ac = Capabilities::new(Product::Accentum, Version::new(3, 10, 0));
        assert_eq!(
            (ac.eq, ac.noise),
            (EqGen::Gen1, NoiseStyle::AncOrTransparency)
        );
        assert_eq!(ac.sidetone_max, 2);
        assert!(!ac.on_head_detection && !ac.touch_lock && !ac.eq_off);
        let tw3 = Capabilities::new(Product::MomentumTw3, Version::new(2, 2, 0));
        assert_eq!((tw3.eq, tw3.noise), (EqGen::Unsupported, NoiseStyle::None));
        let tw4 = Capabilities::new(Product::MomentumTw4, Version::new(5, 3, 0));
        assert_eq!((tw4.eq, tw4.sidetone_max), (EqGen::Gen2, 3));
        assert!(tw4.transparency_switch && !tw4.anti_wind && !tw4.on_head_detection);
        let sport = Capabilities::new(Product::MomentumSport, Version::new(3, 16, 0));
        assert_eq!(
            (sport.eq, sport.noise),
            (EqGen::Gen1, NoiseStyle::SportTile)
        );
        assert!(!sport.touch_lock && !sport.hi_res_audio);
    }
}
