//! Command ids from `re/device-model/m4.json` and its feature files (see PROTOCOL.md §4).
//! Only the base *command* ids are listed; response = `| 0x0100`, notification = `| 0x0080`, error = `| 0x0180`.

pub const VENDOR_SENNHEISER: u16 = 0x0495;
pub const VENDOR_QUALCOMM: u16 = 0x001D;

pub const RESPONSE_BIT: u16 = 0x0100;
pub const NOTIFICATION_BIT: u16 = 0x0080;

/// Sennheiser vendor (0x0495) commands.
pub mod sn {
    // core (feature 0)
    pub const REGISTER_NOTIFICATION: u16 = 0x0007;
    pub const FACTORY_RESET: u16 = 0x0040;
    pub const PRIMARY_SIDE_GET: u16 = 0x0041;
    // device (feature 2)
    pub const ON_HEAD_DETECTION_SET: u16 = 0x0400;
    pub const ON_HEAD_DETECTION_GET: u16 = 0x0401;
    pub const PHYSICAL_STATE_GET: u16 = 0x0402;
    pub const BT_COMPAT_MODE_SET: u16 = 0x0405;
    pub const BT_COMPAT_MODE_GET: u16 = 0x0406;
    // battery / power (feature 3)
    pub const TIMER_SET: u16 = 0x0600;
    pub const TIMER_GET: u16 = 0x0601;
    pub const CHARGING_STATE_GET: u16 = 0x0602;
    pub const BATTERY_LEVEL_GET: u16 = 0x0603;
    // generic audio (feature 4)
    pub const CODEC_GET: u16 = 0x0800;
    pub const PROMPT_MODE_SET: u16 = 0x0801;
    pub const PROMPT_MODE_GET: u16 = 0x0802;
    pub const AUDIO_MODE_SET: u16 = 0x0803;
    pub const AUDIO_MODE_GET: u16 = 0x0804;
    pub const SIDETONE_SET: u16 = 0x0805;
    pub const SIDETONE_GET: u16 = 0x0806;
    pub const PROMPT_LANGUAGE_GET: u16 = 0x0807;
    pub const PROMPT_LANGUAGES_AVAILABLE_GET: u16 = 0x0808;
    pub const AUTO_CALL_SET: u16 = 0x080A;
    pub const AUTO_CALL_GET: u16 = 0x080B;
    pub const SMART_PAUSE_SET: u16 = 0x080C;
    pub const SMART_PAUSE_GET: u16 = 0x080D;
    pub const COMFORT_CALL_SET: u16 = 0x0814;
    pub const COMFORT_CALL_GET: u16 = 0x0815;
    pub const LOW_LATENCY_SET: u16 = 0x0817;
    pub const LOW_LATENCY_GET: u16 = 0x0818;
    /// Not in the app model; observed on fw 3.38 as notification 0x089A carrying a UINT32 sample rate (Hz).
    pub const SAMPLE_RATE_GET: u16 = 0x081A;
    // user EQ (feature 8)
    pub const EQ2_CONFIG_GET: u16 = 0x1000;
    pub const EQ2_SLIDER_SET: u16 = 0x1001;
    pub const EQ2_SLIDER_GET: u16 = 0x1002; // NB: Gen1 uses 0x1002 for EQ_UserParameterGroup_Set
    pub const EQ1_PARAM_GROUP_SET: u16 = 0x1002;
    pub const EQ1_PARAM_GROUP_GET: u16 = 0x1003;
    pub const EQ1_KNOB_SET: u16 = 0x1004;
    pub const EQ1_KNOB_GET: u16 = 0x1005;
    pub const EQ1_LEVEL_SHIFT_SET: u16 = 0x1006;
    pub const EQ1_LEVEL_SHIFT_GET: u16 = 0x1007;
    pub const EQ2_SUBMODE_SET: u16 = 0x1008;
    pub const EQ2_SUBMODE_GET: u16 = 0x1009;
    // versions (feature 9)
    pub const HW_REVISION_GET: u16 = 0x1200;
    pub const SYSTEM_RELEASE_VERSION_GET: u16 = 0x1201;
    pub const COMPONENT_VERSIONS_GET: u16 = 0x1202;
    pub const MODEL_ID_GET: u16 = 0x1206;
    // device management (feature 10)
    pub const PDL_SIZE_GET: u16 = 0x1400;
    pub const PDL_INFO_GET: u16 = 0x1401;
    pub const PDL_CONNECT: u16 = 0x1402;
    pub const PDL_DISCONNECT: u16 = 0x1403;
    pub const PDL_CONNECTION_STATUS_GET: u16 = 0x1404;
    pub const PDL_DELETE_ENTRY: u16 = 0x1405;
    pub const PDL_DELETE_ALL: u16 = 0x1406;
    pub const PDL_OWN_INDEX_GET: u16 = 0x1407;
    pub const PDL_MAX_CONNECTIONS_GET: u16 = 0x1409;
    // mmi (feature 11)
    pub const MMI_SET_CONFIG: u16 = 0x1600;
    pub const MMI_GET_CONFIG: u16 = 0x1601;
    pub const MMI_SET_DEFAULT: u16 = 0x1604;
    pub const MMI_IS_DEFAULT: u16 = 0x1605;
    pub const TOUCH_LOCK_SET: u16 = 0x1606;
    pub const TOUCH_LOCK_GET: u16 = 0x1607;
    // transparent hearing (feature 12)
    pub const TH_PAUSE_ON_FULL_SET: u16 = 0x1800;
    pub const TH_PAUSE_ON_FULL_GET: u16 = 0x1801;
    pub const TH_STATUS_SET: u16 = 0x1804;
    pub const TH_STATUS_GET: u16 = 0x1805;
    // ANC (feature 13)
    pub const ANC_MODE_SET: u16 = 0x1A00;
    pub const ANC_MODE_GET: u16 = 0x1A01;
    pub const ANC_TRANSPARENCY_SET: u16 = 0x1A02;
    pub const ANC_TRANSPARENCY_GET: u16 = 0x1A03;
    pub const ANC_STATUS_SET: u16 = 0x1A04;
    pub const ANC_STATUS_GET: u16 = 0x1A05;
    // personalized sound (feature 16)
    pub const PS_MODE_SET: u16 = 0x2000;
    pub const PS_MODE_GET: u16 = 0x2001;
}

/// Qualcomm vendor (0x001D) commands.
pub mod qc {
    pub const API_VERSION_GET: u16 = 0x0000;
    pub const SUPPORTED_FEATURES_GET: u16 = 0x0001;
    pub const SUPPORTED_FEATURES_NEXT_GET: u16 = 0x0002;
    pub const SERIAL_NUMBER_GET: u16 = 0x0003;
    pub const REGISTER_NOTIFICATION: u16 = 0x0007;
    pub const DATA_TRANSFER_SETUP: u16 = 0x0009;
    pub const DATA_TRANSFER_GET: u16 = 0x000A;
    pub const TRANSPORT_INFO_GET: u16 = 0x000C;
    pub const TRANSPORT_PARAMETER_SET: u16 = 0x000D;
}

/// Sennheiser notification feature ids for `REGISTER_NOTIFICATION`.
pub mod feature {
    pub const CORE: u8 = 0;
    pub const UPGRADE: u8 = 1;
    pub const DEVICE: u8 = 2;
    pub const BATTERY: u8 = 3;
    pub const GENERIC_AUDIO: u8 = 4;
    pub const USER_EQ: u8 = 8;
    pub const VERSIONS: u8 = 9;
    pub const DEVICE_MANAGEMENT: u8 = 10;
    pub const MMI: u8 = 11;
    pub const TRANSPARENT_HEARING: u8 = 12;
    pub const ANC: u8 = 13;
    pub const PERSONALIZED_SOUND: u8 = 16;
}
