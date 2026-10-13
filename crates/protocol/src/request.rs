//! Typed requests (host → headset).

use crate::frame::Packet;
use crate::ids::{VENDOR_QUALCOMM, VENDOR_SENNHEISER, qc, sn};
use crate::types::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    // identity
    FirmwareVersion,
    ModelId,
    SerialNumber,
    HwRevision,
    ComponentVersions,
    // power
    Battery,
    Charging,
    Timer(u8),
    SetTimer {
        id: u8,
        seconds: u16,
    },
    // noise control
    AncModes,
    SetAncMode {
        mode: u8,
        state: u8,
    },
    AncStatus,
    SetAncStatus(bool),
    Transparency,
    SetTransparency(u8),
    PauseOnFullTransparency,
    SetPauseOnFullTransparency(bool),
    Sidetone,
    SetSidetone(u8),
    // audio & behaviour
    Codec,
    AudioMode,
    SetAudioMode(AudioMode),
    PromptMode,
    SetPromptMode(PromptMode),
    PromptLanguage,
    OnHeadDetection,
    SetOnHeadDetection(bool),
    SmartPause,
    SetSmartPause(bool),
    AutoCall,
    SetAutoCall(bool),
    ComfortCall,
    SetComfortCall(bool),
    TouchLock,
    SetTouchLock(bool),
    BtMode,
    SetBtMode(BtMode),
    LowLatency,
    SetLowLatency(bool),
    PhysicalState,
    // EQ gen 2
    Eq2Config,
    Eq2Slider(u8),
    SetEq2Slider {
        band: u8,
        gain_x10: i8,
    },
    Eq2SubMode,
    SetEq2SubMode(u8),
    // EQ gen 1
    Eq1LevelShift,
    SetEq1LevelShift(i8),
    Eq1Knob(u16),
    SetEq1Knob {
        index: u16,
        value: u16,
    },
    Eq1Params(Vec<i16>),
    SetEq1Params(Vec<i16>),
    // personalized sound
    PersonalizedSound,
    SetPersonalizedSound(PersonalizedSoundMode),
    // paired devices
    PairedCount,
    PairedInfo(u8),
    PairedOwnIndex,
    PairedMaxConnections,
    PairedConnect(u8),
    PairedDisconnect(u8),
    /// Removes one host from the headset's pairing list (`DeviceList_DeleteEntry`). Deny-listed for raw use; the
    /// engine only sends it for a confirmed "Forget device" on a host that isn't this PC.
    PairedDeleteEntry(u8),
    /// Restores factory settings and removes all pairings. Deny-listed for raw use; the engine only sends it for
    /// an explicit, confirmed user action.
    FactoryReset,
    // misc
    RegisterNotifications(u8),
    ApiVersion,
    /// Escape hatch for tooling. The engine refuses dangerous raw packets unless explicitly unlocked.
    Raw(Packet),
}

fn b(v: bool) -> u8 {
    u8::from(v)
}

impl Request {
    pub fn packet(&self) -> Packet {
        use Request::*;
        let s = |id: u16, payload: Vec<u8>| Packet::new(VENDOR_SENNHEISER, id, payload);
        let g = |id: u16| Packet::new(VENDOR_SENNHEISER, id, Vec::new());
        match self {
            FirmwareVersion => g(sn::SYSTEM_RELEASE_VERSION_GET),
            ModelId => g(sn::MODEL_ID_GET),
            SerialNumber => Packet::new(VENDOR_QUALCOMM, qc::SERIAL_NUMBER_GET, []),
            HwRevision => g(sn::HW_REVISION_GET),
            ComponentVersions => g(sn::COMPONENT_VERSIONS_GET),
            Battery => g(sn::BATTERY_LEVEL_GET),
            Charging => g(sn::CHARGING_STATE_GET),
            Timer(id) => s(sn::TIMER_GET, vec![*id]),
            SetTimer { id, seconds } => {
                let [hi, lo] = seconds.to_be_bytes();
                s(sn::TIMER_SET, vec![*id, hi, lo])
            }
            AncModes => g(sn::ANC_MODE_GET),
            SetAncMode { mode, state } => s(sn::ANC_MODE_SET, vec![*mode, *state]),
            AncStatus => g(sn::ANC_STATUS_GET),
            SetAncStatus(on) => s(sn::ANC_STATUS_SET, vec![b(*on)]),
            Transparency => g(sn::ANC_TRANSPARENCY_GET),
            SetTransparency(level) => s(sn::ANC_TRANSPARENCY_SET, vec![(*level).min(100)]),
            PauseOnFullTransparency => g(sn::TH_PAUSE_ON_FULL_GET),
            SetPauseOnFullTransparency(on) => s(sn::TH_PAUSE_ON_FULL_SET, vec![b(*on)]),
            Sidetone => g(sn::SIDETONE_GET),
            SetSidetone(v) => s(sn::SIDETONE_SET, vec![*v]),
            Codec => g(sn::CODEC_GET),
            AudioMode => g(sn::AUDIO_MODE_GET),
            SetAudioMode(m) => s(sn::AUDIO_MODE_SET, m.to_u16().to_be_bytes().to_vec()),
            PromptMode => g(sn::PROMPT_MODE_GET),
            SetPromptMode(m) => s(sn::PROMPT_MODE_SET, vec![m.to_u8()]),
            PromptLanguage => g(sn::PROMPT_LANGUAGE_GET),
            OnHeadDetection => g(sn::ON_HEAD_DETECTION_GET),
            SetOnHeadDetection(on) => s(sn::ON_HEAD_DETECTION_SET, vec![b(*on)]),
            SmartPause => g(sn::SMART_PAUSE_GET),
            SetSmartPause(on) => s(sn::SMART_PAUSE_SET, vec![b(*on)]),
            AutoCall => g(sn::AUTO_CALL_GET),
            SetAutoCall(on) => s(sn::AUTO_CALL_SET, vec![b(*on)]),
            ComfortCall => g(sn::COMFORT_CALL_GET),
            SetComfortCall(on) => s(sn::COMFORT_CALL_SET, vec![b(*on)]),
            TouchLock => g(sn::TOUCH_LOCK_GET),
            SetTouchLock(on) => s(sn::TOUCH_LOCK_SET, vec![b(*on)]),
            BtMode => g(sn::BT_COMPAT_MODE_GET),
            SetBtMode(m) => s(sn::BT_COMPAT_MODE_SET, vec![m.to_u8()]),
            LowLatency => g(sn::LOW_LATENCY_GET),
            SetLowLatency(on) => s(sn::LOW_LATENCY_SET, vec![b(*on)]),
            PhysicalState => g(sn::PHYSICAL_STATE_GET),
            Eq2Config => g(sn::EQ2_CONFIG_GET),
            Eq2Slider(band) => s(sn::EQ2_SLIDER_GET, vec![*band]),
            SetEq2Slider { band, gain_x10 } => s(sn::EQ2_SLIDER_SET, vec![*band, *gain_x10 as u8]),
            Eq2SubMode => g(sn::EQ2_SUBMODE_GET),
            SetEq2SubMode(m) => s(sn::EQ2_SUBMODE_SET, vec![*m]),
            Eq1LevelShift => g(sn::EQ1_LEVEL_SHIFT_GET),
            SetEq1LevelShift(v) => s(sn::EQ1_LEVEL_SHIFT_SET, vec![*v as u8]),
            Eq1Knob(index) => s(sn::EQ1_KNOB_GET, index.to_be_bytes().to_vec()),
            SetEq1Knob { index, value } => {
                let mut p = index.to_be_bytes().to_vec();
                p.extend_from_slice(&value.to_be_bytes());
                s(sn::EQ1_KNOB_SET, p)
            }
            Eq1Params(q) => s(sn::EQ1_PARAM_GROUP_GET, i16s(q)),
            SetEq1Params(p) => s(sn::EQ1_PARAM_GROUP_SET, i16s(p)),
            PersonalizedSound => g(sn::PS_MODE_GET),
            SetPersonalizedSound(m) => s(sn::PS_MODE_SET, vec![m.to_u8()]),
            PairedCount => g(sn::PDL_SIZE_GET),
            PairedInfo(i) => s(sn::PDL_INFO_GET, vec![*i]),
            PairedOwnIndex => g(sn::PDL_OWN_INDEX_GET),
            PairedMaxConnections => g(sn::PDL_MAX_CONNECTIONS_GET),
            PairedConnect(i) => s(sn::PDL_CONNECT, vec![*i]),
            FactoryReset => g(sn::FACTORY_RESET),
            PairedDisconnect(i) => s(sn::PDL_DISCONNECT, vec![*i]),
            PairedDeleteEntry(i) => s(sn::PDL_DELETE_ENTRY, vec![*i]),
            RegisterNotifications(feature) => s(sn::REGISTER_NOTIFICATION, vec![*feature]),
            ApiVersion => Packet::new(VENDOR_QUALCOMM, qc::API_VERSION_GET, []),
            Raw(p) => p.clone(),
        }
    }

    /// Commands that can brick, reset, unpair or re-flash the headset. Never sent unless explicitly unlocked.
    pub fn is_dangerous(&self) -> bool {
        is_dangerous_packet(&self.packet())
    }
}

/// Deny-list (PROTOCOL.md §4.9): factory reset, upgrade/DFU, pairing-list wipe, debug & data transfer,
/// transport re-negotiation, MMI rewrites.
pub fn is_dangerous_packet(p: &Packet) -> bool {
    let id = p.command_id();
    match p.vendor {
        VENDOR_SENNHEISER => {
            id == sn::FACTORY_RESET
                || p.feature() == crate::ids::feature::UPGRADE
                || id == sn::PDL_DELETE_ALL
                || id == sn::PDL_DELETE_ENTRY
                || id == sn::MMI_SET_CONFIG
                || id == sn::MMI_SET_DEFAULT
        }
        VENDOR_QUALCOMM => {
            matches!(p.feature(), 4 | 6) // debug, DFU
                || matches!(id, qc::DATA_TRANSFER_SETUP | qc::DATA_TRANSFER_GET | qc::TRANSPORT_PARAMETER_SET)
        }
        _ => true,
    }
}

fn i16s(v: &[i16]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_be_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_typed_requests() {
        let enc = |r: Request| r.packet().encode().unwrap();
        assert_eq!(
            enc(Request::SetAncMode { mode: 1, state: 2 }),
            [0xFF, 3, 0, 2, 0x04, 0x95, 0x1A, 0x00, 1, 2]
        );
        assert_eq!(
            enc(Request::SetTransparency(50)),
            [0xFF, 3, 0, 1, 0x04, 0x95, 0x1A, 0x02, 0x32]
        );
        assert_eq!(enc(Request::SetTransparency(250))[8], 100);
        assert_eq!(
            enc(Request::SetAudioMode(AudioMode::Podcast)),
            [0xFF, 3, 0, 2, 0x04, 0x95, 0x08, 0x03, 0, 2]
        );
        assert_eq!(
            enc(Request::SetEq2Slider {
                band: 2,
                gain_x10: -35
            }),
            [0xFF, 3, 0, 2, 0x04, 0x95, 0x10, 0x01, 2, 0xDD]
        );
        assert_eq!(
            enc(Request::SetTimer {
                id: 0,
                seconds: 1800
            }),
            [0xFF, 3, 0, 3, 0x04, 0x95, 0x06, 0x00, 0, 0x07, 0x08]
        );
        assert_eq!(
            enc(Request::Eq1Knob(1)),
            [0xFF, 3, 0, 2, 0x04, 0x95, 0x10, 0x05, 0, 1]
        );
        assert_eq!(
            enc(Request::SerialNumber),
            [0xFF, 3, 0, 0, 0x00, 0x1D, 0x00, 0x03]
        );
        // m4.json: DeviceList_Connect / _Disconnect / _DeleteEntry take UINT8 deviceIndex; Service_FactoryReset none
        assert_eq!(
            enc(Request::PairedConnect(0)),
            [0xFF, 3, 0, 1, 0x04, 0x95, 0x14, 0x02, 0]
        );
        assert_eq!(
            enc(Request::PairedDisconnect(0)),
            [0xFF, 3, 0, 1, 0x04, 0x95, 0x14, 0x03, 0]
        );
        assert_eq!(
            enc(Request::PairedDeleteEntry(2)),
            [0xFF, 3, 0, 1, 0x04, 0x95, 0x14, 0x05, 2]
        );
        assert_eq!(
            enc(Request::FactoryReset),
            [0xFF, 3, 0, 0, 0x04, 0x95, 0x00, 0x40]
        );
    }

    #[test]
    fn deny_list() {
        let raw = |v, id| Request::Raw(Packet::new(v, id, [])).is_dangerous();
        assert!(raw(VENDOR_SENNHEISER, sn::FACTORY_RESET));
        assert!(raw(VENDOR_SENNHEISER, 0x0200)); // enter DFU
        assert!(raw(VENDOR_SENNHEISER, sn::PDL_DELETE_ALL));
        assert!(raw(VENDOR_QUALCOMM, 0x0C00)); // upgrade connect
        assert!(raw(VENDOR_QUALCOMM, qc::TRANSPORT_PARAMETER_SET));
        assert!(raw(0x1234, 0x0001));
        // single-entry delete and factory reset only go out through the engine's confirmed actions
        assert!(Request::PairedDeleteEntry(2).is_dangerous());
        assert!(Request::FactoryReset.is_dangerous());
        assert!(!Request::PairedConnect(2).is_dangerous());
        assert!(!Request::PairedDisconnect(2).is_dangerous());
        assert!(!Request::SetTransparency(10).is_dangerous());
        assert!(!Request::Battery.is_dangerous());
        assert!(!Request::RegisterNotifications(13).is_dangerous());
    }
}
