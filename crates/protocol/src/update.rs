//! Decoding headset → host packets (responses and notifications) into state updates.

use crate::frame::{Kind, Packet};
use crate::ids::{VENDOR_QUALCOMM, VENDOR_SENNHEISER, qc, sn};
use crate::request::Request;
use crate::types::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Update {
    FirmwareVersion(Version),
    ModelId(String),
    SerialNumber(String),
    HwRevision(u16),
    ComponentVersions(Vec<u8>),
    /// Battery percentage; `None` when the headset reports 255 (unknown).
    Battery(Option<u8>),
    Charging(ChargingState),
    Timer {
        id: u8,
        seconds: u16,
    },
    AntiWind(AntiWind),
    Adaptive(bool),
    /// All ANC mode/state pairs as reported (includes modes we do not interpret).
    AncPairs(Vec<(u8, u8)>),
    AncStatus(bool),
    Transparency(u8),
    PauseOnFullTransparency(bool),
    Sidetone(u8),
    Codec(Option<Codec>),
    SampleRate(u32),
    TransparentHearingStatus(bool),
    AudioMode(AudioMode),
    PromptMode(PromptMode),
    PromptLanguage(u8),
    OnHeadDetection(bool),
    SmartPause(bool),
    AutoCall(bool),
    ComfortCall(bool),
    TouchLock(bool),
    BtMode(BtMode),
    LowLatency(bool),
    PhysicalState(PhysicalState),
    Eq2Config {
        bands: u8,
        min_x10: i8,
        max_x10: i8,
    },
    Eq2Slider {
        band: u8,
        gain_x10: i8,
    },
    Eq2Sliders(Vec<i8>),
    Eq2SubMode(u8),
    Eq1LevelShift(i8),
    Eq1Knob {
        index: u16,
        value: u16,
    },
    Eq1Params(Vec<i16>),
    PersonalizedSound(Option<PersonalizedSoundMode>),
    PairedCount(u16),
    PairedInfo {
        index: u8,
        priority: u8,
        status: u8,
        name: String,
    },
    PairedOwnIndex(u8),
    PairedMaxConnections(u8),
    PairedConnection {
        index: u8,
        status: u8,
    },
    ApiVersion(u8, u8),
    /// A set command was acknowledged.
    Ack(u16),
    Error {
        command: u16,
        code: u8,
    },
}

fn u16_at(p: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*p.get(i)?, *p.get(i + 1)?]))
}

fn string(p: &[u8]) -> String {
    let end = p.iter().position(|&c| c == 0).unwrap_or(p.len());
    String::from_utf8_lossy(&p[..end]).trim().to_string()
}

fn flag(p: &[u8]) -> Option<bool> {
    p.first().map(|&v| v != 0)
}

/// Decode an ANC mode/state pair list into typed updates.
fn anc_pairs(p: &[u8]) -> Vec<Update> {
    let pairs: Vec<(u8, u8)> = p.chunks_exact(2).map(|c| (c[0], c[1])).collect();
    let mut out = Vec::new();
    for &(mode, state) in &pairs {
        match mode {
            anc_mode::ANTI_WIND => out.extend(AntiWind::from_u8(state).map(Update::AntiWind)),
            anc_mode::ADAPTIVE => out.push(Update::Adaptive(state != 0)),
            _ => {}
        }
    }
    out.push(Update::AncPairs(pairs));
    out
}

/// Decode a packet received from the headset. `request` is the in-flight request this packet answers, when known
/// (needed for responses whose payload does not identify the queried item, e.g. EQ slider gains).
pub fn decode(p: &Packet, request: Option<&Request>) -> Vec<Update> {
    let kind = p.kind();
    let id = p.command_id();
    if kind == Kind::Error {
        return vec![Update::Error {
            command: id,
            code: p.error_code().unwrap_or(0),
        }];
    }
    if kind == Kind::Command {
        return Vec::new();
    }
    let d = p.payload.as_slice();
    let one = |u: Option<Update>| u.into_iter().collect::<Vec<_>>();

    if p.vendor == VENDOR_QUALCOMM {
        return match id {
            qc::SERIAL_NUMBER_GET => vec![Update::SerialNumber(string(d))],
            qc::API_VERSION_GET if d.len() >= 2 => vec![Update::ApiVersion(d[0], d[1])],
            _ if d.is_empty() && kind == Kind::Response => vec![Update::Ack(id)],
            _ => Vec::new(),
        };
    }
    if p.vendor != VENDOR_SENNHEISER {
        return Vec::new();
    }

    match id {
        sn::SYSTEM_RELEASE_VERSION_GET => one((|| {
            Some(Update::FirmwareVersion(Version::new(
                u16_at(d, 0)?,
                u16_at(d, 2)?,
                u16_at(d, 4)?,
            )))
        })()),
        sn::MODEL_ID_GET => vec![Update::ModelId(string(d))],
        sn::HW_REVISION_GET => one(u16_at(d, 0).map(Update::HwRevision)),
        sn::COMPONENT_VERSIONS_GET => vec![Update::ComponentVersions(d.to_vec())],
        sn::BATTERY_LEVEL_GET => one(d.first().map(|&v| Update::Battery((v <= 100).then_some(v)))),
        sn::CHARGING_STATE_GET => one(d
            .first()
            .and_then(|&v| ChargingState::from_u8(v))
            .map(Update::Charging)),
        sn::TIMER_GET if d.len() >= 3 => vec![Update::Timer {
            id: d[0],
            seconds: u16_at(d, 1).unwrap(),
        }],
        sn::ANC_MODE_GET => anc_pairs(d),
        sn::ANC_STATUS_GET => one(flag(d).map(Update::AncStatus)),
        sn::ANC_TRANSPARENCY_GET => one(d.first().map(|&v| Update::Transparency(v.min(100)))),
        sn::TH_PAUSE_ON_FULL_GET => one(flag(d).map(Update::PauseOnFullTransparency)),
        sn::SIDETONE_GET => one(d.first().map(|&v| Update::Sidetone(v))),
        sn::CODEC_GET => one(d.first().map(|&v| Update::Codec(Codec::from_u8(v)))),
        sn::SAMPLE_RATE_GET if d.len() >= 4 => vec![Update::SampleRate(u32::from_be_bytes([
            d[0], d[1], d[2], d[3],
        ]))],
        sn::TH_STATUS_GET => one(flag(d).map(Update::TransparentHearingStatus)),
        sn::AUDIO_MODE_GET => one(u16_at(d, 0)
            .or(d.first().map(|&v| v as u16))
            .map(|v| Update::AudioMode(AudioMode::from_u16(v)))),
        sn::PROMPT_MODE_GET => one(d
            .first()
            .and_then(|&v| PromptMode::from_u8(v))
            .map(Update::PromptMode)),
        sn::PROMPT_LANGUAGE_GET => one(d.first().map(|&v| Update::PromptLanguage(v))),
        sn::ON_HEAD_DETECTION_GET => one(flag(d).map(Update::OnHeadDetection)),
        sn::SMART_PAUSE_GET => one(flag(d).map(Update::SmartPause)),
        sn::AUTO_CALL_GET => one(flag(d).map(Update::AutoCall)),
        sn::COMFORT_CALL_GET => one(flag(d).map(Update::ComfortCall)),
        sn::TOUCH_LOCK_GET => one(flag(d).map(Update::TouchLock)),
        sn::BT_COMPAT_MODE_GET => one(d
            .first()
            .and_then(|&v| BtMode::from_u8(v))
            .map(Update::BtMode)),
        sn::LOW_LATENCY_GET => one(flag(d).map(Update::LowLatency)),
        sn::PHYSICAL_STATE_GET => one(d
            .first()
            .and_then(|&v| PhysicalState::from_u8(v))
            .map(Update::PhysicalState)),
        sn::EQ2_CONFIG_GET if d.len() >= 3 => {
            vec![Update::Eq2Config {
                bands: d[0],
                min_x10: d[1] as i8,
                max_x10: d[2] as i8,
            }]
        }
        // 0x1002 is "get slider gain" (Gen2) or "set parameter group" (Gen1): disambiguate via the request.
        sn::EQ2_SLIDER_GET => match (kind, request) {
            (Kind::Notification, _) => {
                vec![Update::Eq2Sliders(d.iter().map(|&v| v as i8).collect())]
            }
            (_, Some(Request::Eq2Slider(band))) => one(d.first().map(|&g| Update::Eq2Slider {
                band: *band,
                gain_x10: g as i8,
            })),
            _ if d.is_empty() => vec![Update::Ack(id)],
            _ => Vec::new(),
        },
        sn::EQ2_SLIDER_SET if kind == Kind::Notification => {
            vec![Update::Eq2Sliders(d.iter().map(|&v| v as i8).collect())]
        }
        sn::EQ2_SUBMODE_GET => one(d.first().map(|&v| Update::Eq2SubMode(v))),
        sn::EQ1_LEVEL_SHIFT_GET => one(d.first().map(|&v| Update::Eq1LevelShift(v as i8))),
        sn::EQ1_KNOB_GET => match request {
            Some(Request::Eq1Knob(index)) => one(u16_at(d, 0).map(|value| Update::Eq1Knob {
                index: *index,
                value,
            })),
            _ => Vec::new(),
        },
        sn::EQ1_PARAM_GROUP_GET => {
            vec![Update::Eq1Params(
                d.chunks_exact(2)
                    .map(|c| i16::from_be_bytes([c[0], c[1]]))
                    .collect(),
            )]
        }
        sn::PS_MODE_GET => one(d
            .first()
            .map(|&v| Update::PersonalizedSound(PersonalizedSoundMode::from_u8(v)))),
        sn::PDL_SIZE_GET => one(u16_at(d, 0).map(Update::PairedCount)),
        sn::PDL_INFO_GET if d.len() >= 3 => {
            vec![Update::PairedInfo {
                index: d[0],
                priority: d[1],
                status: d[2],
                name: string(&d[3..]),
            }]
        }
        sn::PDL_OWN_INDEX_GET => one(d.first().map(|&v| Update::PairedOwnIndex(v))),
        sn::PDL_MAX_CONNECTIONS_GET => one(d.first().map(|&v| Update::PairedMaxConnections(v))),
        sn::PDL_CONNECTION_STATUS_GET if d.len() >= 2 => vec![Update::PairedConnection {
            index: d[0],
            status: d[1],
        }],
        _ if kind == Kind::Response && d.is_empty() => vec![Update::Ack(id)],
        _ => Vec::new(),
    }
}

impl Request {
    /// The state change a *set* request implies once acknowledged (for optimistic UI and for devices that do
    /// not echo a notification).
    pub fn implied_updates(&self) -> Vec<Update> {
        use Request::*;
        match self {
            SetTimer { id, seconds } => vec![Update::Timer {
                id: *id,
                seconds: *seconds,
            }],
            SetAncMode { mode, state } => match *mode {
                anc_mode::ANTI_WIND => AntiWind::from_u8(*state)
                    .map(Update::AntiWind)
                    .into_iter()
                    .collect(),
                anc_mode::ADAPTIVE => vec![Update::Adaptive(*state != 0)],
                _ => Vec::new(),
            },
            SetAncStatus(v) => vec![Update::AncStatus(*v)],
            SetTransparency(v) => vec![Update::Transparency((*v).min(100))],
            SetPauseOnFullTransparency(v) => vec![Update::PauseOnFullTransparency(*v)],
            SetSidetone(v) => vec![Update::Sidetone(*v)],
            SetAudioMode(m) => vec![Update::AudioMode(*m)],
            SetPromptMode(m) => vec![Update::PromptMode(*m)],
            SetOnHeadDetection(v) => vec![Update::OnHeadDetection(*v)],
            SetSmartPause(v) => vec![Update::SmartPause(*v)],
            SetAutoCall(v) => vec![Update::AutoCall(*v)],
            SetComfortCall(v) => vec![Update::ComfortCall(*v)],
            SetTouchLock(v) => vec![Update::TouchLock(*v)],
            SetBtMode(m) => vec![Update::BtMode(*m)],
            SetLowLatency(v) => vec![Update::LowLatency(*v)],
            SetEq2Slider { band, gain_x10 } => vec![Update::Eq2Slider {
                band: *band,
                gain_x10: *gain_x10,
            }],
            SetEq2SubMode(m) => vec![Update::Eq2SubMode(*m)],
            SetEq1LevelShift(v) => vec![Update::Eq1LevelShift(*v)],
            SetEq1Knob { index, value } => vec![Update::Eq1Knob {
                index: *index,
                value: *value,
            }],
            SetPersonalizedSound(m) => vec![Update::PersonalizedSound(Some(*m))],
            _ => Vec::new(),
        }
    }

    /// The response id expected for this request.
    pub fn response_id(&self) -> u16 {
        self.packet().id | crate::ids::RESPONSE_BIT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Deframer;

    fn rx(hex: &str, req: Option<&Request>) -> Vec<Update> {
        let bytes: Vec<u8> = hex
            .split_whitespace()
            .map(|h| u8::from_str_radix(h, 16).unwrap())
            .collect();
        let p = Deframer::new().push(&bytes);
        assert_eq!(p.len(), 1, "{hex}");
        decode(&p[0], req)
    }

    /// Real responses captured from a MOMENTUM 4 (fw 3.38.3) during Phase 0.
    #[test]
    fn decodes_captured_m4_responses() {
        assert_eq!(
            rx("FF 03 00 06 04 95 13 01 00 03 00 26 00 03", None),
            vec![Update::FirmwareVersion(Version::new(3, 38, 3))]
        );
        assert_eq!(
            rx(
                "FF 03 00 0D 04 95 13 06 4D 34 41 45 42 54 20 44 65 6E 69 6D 00",
                None
            ),
            vec![Update::ModelId("M4AEBT Denim".into())]
        );
        assert_eq!(
            rx("FF 03 00 01 04 95 07 03 64", None),
            vec![Update::Battery(Some(100))]
        );
        assert_eq!(
            rx("FF 03 00 02 04 95 07 02 00 00", None),
            vec![Update::Charging(ChargingState::Disconnected)]
        );
        assert_eq!(
            rx("FF 03 00 01 04 95 09 00 01", None),
            vec![Update::Codec(Some(Codec::Aac))]
        );
        assert_eq!(
            rx("FF 03 00 06 04 95 1B 01 01 02 02 00 03 00", None),
            vec![
                Update::AntiWind(AntiWind::Auto),
                Update::Adaptive(false),
                Update::AncPairs(vec![(1, 2), (2, 0), (3, 0)])
            ]
        );
        assert_eq!(
            rx("FF 03 00 01 04 95 1B 05 00", None),
            vec![Update::AncStatus(false)]
        );
        assert_eq!(
            rx("FF 03 00 01 04 95 1B 03 00", None),
            vec![Update::Transparency(0)]
        );
        assert_eq!(
            rx("FF 03 00 01 04 95 19 01 01", None),
            vec![Update::PauseOnFullTransparency(true)]
        );
        assert_eq!(
            rx("FF 03 00 01 04 95 09 06 02", None),
            vec![Update::Sidetone(2)]
        );
        assert_eq!(
            rx("FF 03 00 02 04 95 09 04 00 00", None),
            vec![Update::AudioMode(AudioMode::Off)]
        );
        assert_eq!(
            rx("FF 03 00 01 04 95 11 87 01", None),
            vec![Update::Error {
                command: 0x1007,
                code: 1
            }]
        );
    }

    #[test]
    fn notifications_and_context_dependent_responses() {
        assert_eq!(
            rx("FF 03 00 01 04 95 1A 83 4B", None),
            vec![Update::Transparency(75)]
        );
        assert_eq!(
            rx("FF 03 00 01 04 95 06 83 FF", None),
            vec![Update::Battery(None)]
        );
        assert_eq!(
            rx("FF 03 00 01 04 95 11 02 E2", Some(&Request::Eq2Slider(3))),
            vec![Update::Eq2Slider {
                band: 3,
                gain_x10: -30
            }]
        );
        assert_eq!(
            rx("FF 03 00 05 04 95 10 82 00 0A EC 3C C4", None),
            vec![Update::Eq2Sliders(vec![0, 10, -20, 60, -60])]
        );
        assert_eq!(
            rx(
                "FF 03 00 00 04 95 11 02",
                Some(&Request::SetEq1Params(vec![]))
            ),
            vec![Update::Ack(0x1002)]
        );
        assert_eq!(
            rx("FF 03 00 00 04 95 1B 02", None),
            vec![Update::Ack(0x1A02)]
        );
        assert_eq!(
            rx("FF 03 00 03 04 95 07 01 00 03 84", None),
            vec![Update::Timer {
                id: 0,
                seconds: 900
            }]
        );
        assert_eq!(
            rx("FF 03 00 04 04 95 08 9A 00 00 BB 80", None),
            vec![Update::SampleRate(48000)]
        );
    }
}
