//! What the app sends: the reads after connecting, and the writes behind each UI action.

use protocol::ids::feature;
use protocol::{AudioMode, BtMode, Capabilities, EqGen, NoiseStyle, Request, anc_mode};

use crate::state::{Action, HeadsetState, NoiseMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    High,
    Low,
}

/// Notification registrations and every read behind the UI, right after connecting.
pub fn refresh_requests(c: &Capabilities) -> Vec<Request> {
    let mut r = vec![];
    let mut features = vec![
        feature::DEVICE,
        feature::BATTERY,
        feature::GENERIC_AUDIO,
        feature::DEVICE_MANAGEMENT,
    ];
    features.extend([feature::MMI, feature::TRANSPARENT_HEARING, feature::ANC]);
    if c.eq == EqGen::Gen2 {
        features.push(feature::USER_EQ);
    }
    if c.sound_personalization {
        features.push(feature::PERSONALIZED_SOUND);
    }
    r.extend(features.into_iter().map(Request::RegisterNotifications));
    r.extend([Request::Battery, Request::Charging, Request::Codec]);
    r.extend(noise_reads(c));
    r.push(Request::AudioMode);
    if c.sidetone_max > 0 {
        r.push(Request::Sidetone);
    }
    if c.comfort_call {
        r.push(Request::ComfortCall);
    }
    if c.voice_prompts {
        r.push(Request::PromptMode);
    }
    if c.on_head_detection {
        r.push(Request::OnHeadDetection);
    }
    if c.smart_pause {
        r.push(Request::SmartPause);
    }
    if c.auto_power_off {
        r.push(Request::Timer(0));
    }
    if c.auto_answer_calls {
        r.push(Request::AutoCall);
    }
    if c.touch_lock {
        r.push(Request::TouchLock);
    }
    if c.hi_res_audio {
        r.push(Request::BtMode);
    }
    if c.physical_state {
        r.push(Request::PhysicalState);
    }
    if c.sound_personalization {
        r.push(Request::PersonalizedSound);
    }
    match c.eq {
        EqGen::Gen2 => {
            r.push(Request::Eq2Config);
            r.extend((0..protocol::eq::BANDS as u8).map(Request::Eq2Slider));
            r.push(Request::Eq2SubMode);
        }
        EqGen::Gen1 => {
            r.extend([
                Request::Eq1LevelShift,
                Request::Eq1Knob(protocol::eq::KNOB_Y_INDEX),
                Request::Eq1Params(protocol::eq::gen1_gain_query()),
            ]);
        }
        EqGen::Unsupported => {}
    }
    r.extend([
        Request::PairedOwnIndex,
        Request::PairedMaxConnections,
        Request::PairedCount,
    ]);
    r
}

/// Translate a UI action into GAIA requests given the current headset state.
pub fn plan(a: &Action, hs: &HeadsetState) -> (Vec<Request>, Priority) {
    use Request as R;
    let mut r = Vec::new();
    // with the ANC master switch, every ANC/transparency setting needs ANC switched on first
    let anc_on_first = |r: &mut Vec<Request>| {
        if hs.caps.anc_off && hs.anc_status != Some(true) {
            r.push(R::SetAncStatus(true));
        }
    };
    let ensure_user_eq = |r: &mut Vec<Request>| {
        if !hs.eq_active() {
            r.push(R::SetAudioMode(AudioMode::UserEq));
        }
    };
    let adaptive = |on: bool| R::SetAncMode {
        mode: anc_mode::ADAPTIVE,
        state: u8::from(on),
    };
    let wind = |on: bool| R::SetAncMode {
        mode: anc_mode::ANTI_WIND,
        state: u8::from(on),
    };
    match a {
        // the other products' tiles, with the writes of the official app's `sendFeatureToDevice`
        Action::SetNoiseMode(m) if hs.caps.noise != NoiseStyle::AdaptiveBalance => {
            if !hs.noise_modes().contains(m) {
                return (r, Priority::High);
            }
            match (hs.caps.noise, m) {
                (NoiseStyle::AncOrTransparency, _) => r.push(R::SetTransparentHearingStatus(
                    *m == NoiseMode::Transparency,
                )),
                (NoiseStyle::AncTile, NoiseMode::Off) => r.push(R::SetAncStatus(false)),
                (NoiseStyle::AncTile, _) => {
                    r.extend([wind(*m == NoiseMode::Outdoor), R::SetAncStatus(true)])
                }
                (NoiseStyle::SportTile, NoiseMode::Transparency) => {
                    r.push(R::SetTransparentHearingStatus(true))
                }
                (NoiseStyle::SportTile, _) => {
                    // the app writes nothing for transparency here; read back what the headset switched off
                    r.extend([
                        R::SetAncStatus(true),
                        wind(*m == NoiseMode::Outdoor),
                        R::TransparentHearingStatus,
                    ])
                }
                _ => {}
            }
        }
        // M4 / ACCENTUM Plus; command order follows Smart Control Plus (`Xna` mode setters)
        Action::SetNoiseMode(NoiseMode::Adaptive) => {
            anc_on_first(&mut r);
            r.push(adaptive(true));
        }
        Action::SetNoiseMode(NoiseMode::Custom) => {
            // the headset keeps its own Custom level; Smart Control Plus doesn't resend it
            r.push(adaptive(false));
            anc_on_first(&mut r);
        }
        Action::SetNoiseMode(NoiseMode::Off) => {
            if hs.caps.anc_off {
                if hs.adaptive != Some(false) {
                    r.push(adaptive(false));
                }
                r.push(R::SetAncStatus(false));
            }
        }
        Action::SetNoiseMode(_) => {}
        Action::SetTransparency(_) if hs.caps.noise != NoiseStyle::AdaptiveBalance => {}
        Action::SetTransparency(t) => {
            // picking a Custom level (e.g. from the tray) puts the headset in Custom mode
            if hs.adaptive != Some(false) {
                r.push(adaptive(false));
            }
            anc_on_first(&mut r);
            r.push(R::SetTransparency((*t).min(100)));
        }
        Action::SetAntiWind(w) => {
            if hs.caps.anti_wind {
                r.push(R::SetAncMode {
                    mode: anc_mode::ANTI_WIND,
                    state: w.to_u8(),
                })
            }
        }
        Action::SetTransparentHearing(on) => {
            if hs.caps.transparency_switch {
                r.push(R::SetTransparentHearingStatus(*on))
            }
        }
        Action::SetSidetone(v) => {
            if hs.caps.sidetone_max > 0 {
                r.push(R::SetSidetone((*v).min(hs.caps.sidetone_max)))
            }
        }
        Action::SetEqBands(bands) => {
            match hs.caps.eq {
                EqGen::Gen2 => {
                    r = protocol::eq::gen2_requests(
                        hs.eq.known.then_some(&hs.eq.bands),
                        bands,
                        None,
                    )
                }
                EqGen::Gen1 => {
                    r = protocol::eq::gen1_requests(
                        hs.product.gen1_params(),
                        bands,
                        hs.eq.bass_boost,
                    )
                }
                EqGen::Unsupported => {}
            }
            // gains are stored in any mode but only heard in UserEq; switch after setting them
            if !r.is_empty() || !hs.eq_active() {
                ensure_user_eq(&mut r);
            }
        }
        Action::SetBassBoost(v) => {
            match hs.caps.eq {
                EqGen::Gen2 => r.push(R::SetEq2SubMode(u8::from(*v))),
                EqGen::Gen1 => {
                    r = protocol::eq::gen1_requests(hs.product.gen1_params(), &hs.eq.bands, *v)
                }
                EqGen::Unsupported => {}
            }
            if *v {
                ensure_user_eq(&mut r);
            }
        }
        Action::SetEqEnabled(on) => {
            if hs.caps.eq != EqGen::Unsupported && (*on || hs.caps.eq_off) {
                r.push(R::SetAudioMode(if *on {
                    AudioMode::UserEq
                } else {
                    AudioMode::Off
                }));
            }
        }
        Action::SetPodcast(v) => r.push(R::SetAudioMode(if *v {
            AudioMode::Podcast
        } else {
            // back to whatever was playing before podcast mode
            match hs.pre_podcast_mode {
                Some(m) if !m.is_podcast() && (m != AudioMode::Off || hs.caps.eq_off) => m,
                _ => AudioMode::UserEq,
            }
        })),
        Action::SetOnHeadDetection(v) => {
            if !hs.caps.on_head_detection {
                return (r, Priority::High);
            }
            r.push(R::SetOnHeadDetection(*v));
            // the official app re-reads the dependent settings
            r.extend([R::SmartPause, R::Timer(0)]);
            if hs.caps.auto_answer_calls {
                r.push(R::AutoCall);
            }
        }
        Action::SetSmartPause(v) => r.push(R::SetSmartPause(*v)),
        Action::SetAutoCall(v) => r.push(R::SetAutoCall(*v)),
        Action::SetComfortCall(v) => {
            if hs.caps.comfort_call {
                r.push(R::SetComfortCall(*v))
            }
        }
        Action::SetTouchLock(v) => {
            if hs.caps.touch_lock {
                r.push(R::SetTouchLock(*v))
            }
        }
        Action::SetHiRes(on) => r.push(R::SetBtMode(if *on {
            BtMode::BestAudio
        } else {
            BtMode::BestCompatibility
        })),
        Action::SetPromptMode(m) => r.push(R::SetPromptMode(*m)),
        Action::SetAutoOff(s) => r.push(R::SetTimer {
            id: protocol::TIMER_POWER_OFF,
            seconds: *s,
        }),
        Action::SetPersonalizedSound(on) => r.push(R::SetPersonalizedSound(if *on {
            protocol::PersonalizedSoundMode::Enabled
        } else {
            protocol::PersonalizedSoundMode::Disabled
        })),
        Action::ConnectHost(i) => r.extend([R::PairedConnect(*i), R::PairedCount]),
        Action::DisconnectHost(i) => r.extend([R::PairedDisconnect(*i), R::PairedCount]),
        Action::ForgetHost(i) => {
            // never this PC, never a connected host; indices shift afterwards, so re-read the whole list
            let host = hs.paired_hosts.iter().find(|h| h.index == *i);
            if hs.paired_own_index.is_some_and(|own| own != *i)
                && host.is_some_and(|h| !h.connected)
            {
                r.extend([R::PairedDeleteEntry(*i), R::PairedOwnIndex, R::PairedCount]);
            }
        }
        Action::FactoryReset => r.push(R::FactoryReset),
        Action::Refresh => {
            r.extend([R::Battery, R::Charging, R::Codec]);
            r.extend(noise_reads(&hs.caps));
            r.push(R::AudioMode);
            return (r, Priority::Low);
        }
        Action::SelectDevice(_) | Action::Reconnect => {}
    }
    (r, Priority::High)
}

/// Reads behind the noise-control UI of a product (only what its tile uses, so an untested model's error log only
/// shows unexpected rejections).
pub fn noise_reads(c: &Capabilities) -> Vec<Request> {
    use Request as R;
    let mut r = match c.noise {
        NoiseStyle::None => vec![],
        NoiseStyle::AdaptiveBalance => vec![R::AncModes, R::AncStatus, R::Transparency],
        NoiseStyle::AncOrTransparency => vec![R::TransparentHearingStatus],
        NoiseStyle::AncTile => vec![R::AncStatus, R::AncModes],
        NoiseStyle::SportTile => vec![R::AncModes, R::TransparentHearingStatus],
    };
    if c.anti_wind && !r.contains(&R::AncModes) {
        r.push(R::AncModes);
    }
    if c.transparency_switch {
        r.push(R::TransparentHearingStatus);
    }
    r
}

pub fn is_user_change(r: &Request) -> bool {
    matches!(
        r,
        Request::PairedConnect(_)
            | Request::PairedDisconnect(_)
            | Request::PairedDeleteEntry(_)
            | Request::FactoryReset
    ) || format!("{r:?}").starts_with("Set")
}

/// Writes that replace each other while queued: only the latest value matters (slider drags produce many
/// intermediate values). Requests with the same key are interchangeable.
pub fn coalesce_key(r: &Request) -> Option<(u16, u8)> {
    let id = r.packet().id;
    match r {
        Request::SetEq2Slider { band, .. } => Some((id, *band)),
        Request::SetAncMode { mode, .. } => Some((id, *mode)),
        Request::SetTransparency(_)
        | Request::SetSidetone(_)
        | Request::SetEq2SubMode(_)
        | Request::SetEq1Params(_)
        | Request::SetEq1LevelShift(_)
        | Request::SetEq1Knob { .. }
        | Request::SetAudioMode(_) => Some((id, 0)),
        _ => None,
    }
}

/// Reads a response makes possible: the paired-device entries once their count is known.
pub fn follow_ups(update: &protocol::Update) -> Vec<Request> {
    match update {
        protocol::Update::PairedCount(n) => {
            (0..(*n).min(8) as u8).map(Request::PairedInfo).collect()
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::HeadsetState;
    use protocol::{AntiWind, Capabilities, Version};

    fn m4() -> HeadsetState {
        let mut h = HeadsetState {
            caps: Capabilities::for_m4(Version::new(3, 38, 3)),
            ..Default::default()
        };
        h.anc_status = Some(false);
        h.eq.known = true;
        h
    }

    #[test]
    fn plans_noise_modes() {
        // the real headset's state: ANC off (fw 3.38 has the master switch)
        let mut off = m4();
        off.adaptive = Some(true);
        assert_eq!(off.noise_mode(), Some(NoiseMode::Off));
        let adaptive_on = Request::SetAncMode { mode: 3, state: 1 };
        let adaptive_off = Request::SetAncMode { mode: 3, state: 0 };
        assert_eq!(
            plan(&Action::SetNoiseMode(NoiseMode::Adaptive), &off).0,
            vec![Request::SetAncStatus(true), adaptive_on.clone()]
        );
        // Custom keeps the headset's stored level
        assert_eq!(
            plan(&Action::SetNoiseMode(NoiseMode::Custom), &off).0,
            vec![adaptive_off.clone(), Request::SetAncStatus(true)]
        );
        // picking a level leaves adaptive and turns ANC on
        assert_eq!(
            plan(&Action::SetTransparency(40), &off).0,
            vec![
                adaptive_off.clone(),
                Request::SetAncStatus(true),
                Request::SetTransparency(40)
            ]
        );
        // Off from Adaptive also turns adaptive off, like Smart Control Plus
        let mut adaptive = m4();
        adaptive.anc_status = Some(true);
        adaptive.adaptive = Some(true);
        assert_eq!(
            plan(&Action::SetNoiseMode(NoiseMode::Off), &adaptive).0,
            vec![adaptive_off.clone(), Request::SetAncStatus(false)]
        );
        // already in Custom with ANC on: only the level
        let mut custom = m4();
        custom.anc_status = Some(true);
        custom.adaptive = Some(false);
        assert_eq!(custom.noise_mode(), Some(NoiseMode::Custom));
        assert_eq!(
            plan(&Action::SetTransparency(40), &custom).0,
            vec![Request::SetTransparency(40)]
        );
        assert_eq!(
            plan(&Action::SetNoiseMode(NoiseMode::Off), &custom).0,
            vec![Request::SetAncStatus(false)]
        );
        // older firmware without the master switch: Off unavailable, ANC status untouched
        let mut old = custom.clone();
        old.caps = Capabilities::for_m4(Version::new(2, 13, 18));
        assert!(
            plan(&Action::SetNoiseMode(NoiseMode::Off), &old)
                .0
                .is_empty()
        );
        assert_eq!(
            plan(&Action::SetNoiseMode(NoiseMode::Adaptive), &old).0,
            vec![adaptive_on]
        );
    }

    fn other(p: protocol::Product, fw: Version) -> HeadsetState {
        let mut h = HeadsetState::default();
        h.apply(&protocol::Update::Product(p));
        h.apply(&protocol::Update::FirmwareVersion(fw));
        h
    }

    #[test]
    fn plans_other_products_noise_tiles() {
        use protocol::Product as P;
        let wind = |on| Request::SetAncMode {
            mode: 1,
            state: u8::from(on),
        };
        // MOMENTUM True Wireless 4: Off / ANC / Outdoor, transparent hearing separate (`Tw4Product`)
        let mut tw4 = other(P::MomentumTw4, Version::new(5, 3, 0));
        tw4.anc_status = Some(true);
        tw4.anti_wind = Some(AntiWind::Max);
        assert_eq!(tw4.noise_mode(), Some(NoiseMode::Outdoor));
        assert_eq!(
            plan(&Action::SetNoiseMode(NoiseMode::Anc), &tw4).0,
            vec![wind(false), Request::SetAncStatus(true)]
        );
        assert_eq!(
            plan(&Action::SetNoiseMode(NoiseMode::Off), &tw4).0,
            vec![Request::SetAncStatus(false)]
        );
        assert_eq!(
            plan(&Action::SetTransparentHearing(true), &tw4).0,
            vec![Request::SetTransparentHearingStatus(true)]
        );
        // modes the product doesn't have are ignored
        assert!(
            plan(&Action::SetNoiseMode(NoiseMode::Adaptive), &tw4)
                .0
                .is_empty()
        );
        assert!(
            plan(&Action::SetAntiWind(AntiWind::Auto), &tw4)
                .0
                .is_empty()
        );
        assert_eq!(
            plan(&Action::SetSidetone(9), &tw4).0,
            vec![Request::SetSidetone(3)]
        );
        // ACCENTUM: ANC always on, transparency on/off (`Ac1Product`)
        let mut ac = other(P::Accentum, Version::new(3, 20, 0));
        ac.transparent_hearing = Some(false);
        assert_eq!(ac.noise_mode(), Some(NoiseMode::Anc));
        assert_eq!(
            plan(&Action::SetNoiseMode(NoiseMode::Transparency), &ac).0,
            vec![Request::SetTransparentHearingStatus(true)]
        );
        assert!(plan(&Action::SetComfortCall(true), &ac).0.is_empty());
        // MOMENTUM Sport: ANC / Outdoor / Transparency (`MomentumSport1Product`)
        let mut sport = other(P::MomentumSport, Version::new(3, 17, 14));
        sport.transparent_hearing = Some(true);
        assert_eq!(sport.noise_mode(), Some(NoiseMode::Transparency));
        assert_eq!(
            plan(&Action::SetNoiseMode(NoiseMode::Outdoor), &sport).0,
            vec![
                Request::SetAncStatus(true),
                wind(true),
                Request::TransparentHearingStatus
            ]
        );
        // ACCENTUM Plus behaves like the M4 without the ANC master switch
        let acp = other(P::AccentumPlus, Version::new(3, 36, 1));
        assert_eq!(
            acp.noise_modes(),
            vec![NoiseMode::Adaptive, NoiseMode::Custom]
        );
        // audio-mode-1 firmware has no EQ-off mode
        let tw3 = other(P::MomentumTw3, Version::new(3, 0, 0));
        assert!(plan(&Action::SetEqEnabled(false), &tw3).0.is_empty());
    }

    #[test]
    fn plans_eq_gen2_diff() {
        let mut hs = m4();
        hs.audio_mode = Some(AudioMode::UserEq);
        let (r, _) = plan(&Action::SetEqBands([0.0, 0.0, 1.5, 0.0, 0.0]), &hs);
        assert_eq!(
            r,
            vec![Request::SetEq2Slider {
                band: 2,
                gain_x10: 15
            }]
        );
        assert_eq!(
            plan(&Action::SetBassBoost(true), &hs).0,
            vec![Request::SetEq2SubMode(1)]
        );
        // fw 3.38 starts with the EQ off: the gains alone would be silent, so switch to the user EQ after them
        hs.audio_mode = Some(AudioMode::Off);
        assert_eq!(
            plan(&Action::SetEqBands([0.0, 0.0, 1.5, 0.0, 0.0]), &hs).0,
            vec![
                Request::SetEq2Slider {
                    band: 2,
                    gain_x10: 15
                },
                Request::SetAudioMode(AudioMode::UserEq)
            ]
        );
        assert_eq!(
            plan(&Action::SetEqBands([0.0; 5]), &hs).0,
            vec![Request::SetAudioMode(AudioMode::UserEq)],
            "same curve, still needs the mode"
        );
        assert_eq!(
            plan(&Action::SetEqEnabled(false), &hs).0,
            vec![Request::SetAudioMode(AudioMode::Off)]
        );
        // podcast off goes back to the mode before it
        hs.audio_mode = Some(AudioMode::Podcast);
        hs.pre_podcast_mode = Some(AudioMode::Off);
        assert_eq!(
            plan(&Action::SetPodcast(false), &hs).0,
            vec![Request::SetAudioMode(AudioMode::Off)]
        );
        assert_eq!(
            plan(&Action::SetSidetone(9), &m4()).0,
            vec![Request::SetSidetone(4)]
        );
    }

    #[test]
    fn forget_host_only_for_idle_other_hosts() {
        let mut hs = m4();
        hs.paired_own_index = Some(1);
        hs.paired_hosts = vec![
            crate::state::PairedHost {
                index: 0,
                name: "Phone".into(),
                connected: true,
            },
            crate::state::PairedHost {
                index: 1,
                name: "PC".into(),
                connected: true,
            },
            crate::state::PairedHost {
                index: 2,
                name: "Tablet".into(),
                connected: false,
            },
        ];
        assert_eq!(
            plan(&Action::ForgetHost(2), &hs).0,
            vec![
                Request::PairedDeleteEntry(2),
                Request::PairedOwnIndex,
                Request::PairedCount
            ]
        );
        assert!(plan(&Action::ForgetHost(1), &hs).0.is_empty(), "this PC");
        assert!(plan(&Action::ForgetHost(0), &hs).0.is_empty(), "connected");
        assert!(plan(&Action::ForgetHost(7), &hs).0.is_empty(), "unknown");
    }

    #[test]
    fn hi_res_is_better_audio_mode() {
        // Smart Control Plus AptX96kHzSupportMode: wire 0 = on, 1 = off
        assert_eq!(
            plan(&Action::SetHiRes(true), &m4()).0,
            vec![Request::SetBtMode(BtMode::BestAudio)]
        );
        assert_eq!(
            plan(&Action::SetHiRes(false), &m4()).0,
            vec![Request::SetBtMode(BtMode::BestCompatibility)]
        );
        assert_eq!(BtMode::BestAudio.to_u8(), 0);
    }

    #[test]
    fn user_change_detection() {
        assert!(is_user_change(&Request::SetTransparency(1)));
        assert!(!is_user_change(&Request::Battery));
        assert!(!is_user_change(&Request::Timer(0)));
    }
}
