//! Value types for M4 properties (value maps from `m4.json`).

use std::fmt;

macro_rules! byte_enum {
    ($(#[$m:meta])* $name:ident { $($var:ident = $val:expr => $label:expr),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name { $($var),+ }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$var),+];
            pub fn from_u8(v: u8) -> Option<Self> {
                match v { $($val => Some(Self::$var),)+ _ => None }
            }
            pub fn to_u8(self) -> u8 {
                match self { $(Self::$var => $val),+ }
            }
            pub fn label(self) -> &'static str {
                match self { $(Self::$var => $label),+ }
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.label()) }
        }
    };
}

byte_enum!(
    /// ANC mode 1 state: wind noise reduction.
    AntiWind { Off = 0 => "Off", Max = 1 => "Max", Auto = 2 => "Auto" }
);

byte_enum!(ChargingState {
    Disconnected = 0 => "Not charging",
    Charging = 1 => "Charging",
    Complete = 2 => "Charged",
});

byte_enum!(Codec {
    Sbc = 0 => "SBC",
    Aac = 1 => "AAC",
    AptX = 2 => "aptX",
    AptXLl = 3 => "aptX LL",
    Mp3 = 4 => "MP3",
    AptXHd = 5 => "aptX HD",
    Faststream = 6 => "FastStream",
    Lhdc = 7 => "LHDC",
    AptXAdaptive = 8 => "aptX Adaptive",
    AptXLossless = 9 => "aptX Lossless",
    Lc3 = 10 => "LC3",
});

byte_enum!(
    /// Voice prompt / tone setting.
    PromptMode { Off = 0 => "Off", TonesOnly = 1 => "Tones only", VoiceAndTones = 2 => "Voice + tones" }
);

byte_enum!(PromptLanguage {
    English = 0 => "English",
    German = 1 => "German",
    French = 2 => "French",
    Spanish = 3 => "Spanish",
    Chinese = 4 => "Chinese",
    Japanese = 5 => "Japanese",
    Russian = 6 => "Russian",
    Korean = 7 => "Korean",
});

byte_enum!(PhysicalState {
    Unknown = 0 => "Unknown",
    InCase = 1 => "In case",
    NotOnHead = 2 => "Not on head",
    OnHead = 3 => "On head",
});

byte_enum!(
    /// Bluetooth compatibility mode.
    BtMode { BestAudio = 0 => "Best audio", BestCompatibility = 1 => "Best compatibility" }
);

byte_enum!(PersonalizedSoundMode {
    NotParameterized = 0 => "Not set up",
    Disabled = 1 => "Disabled",
    Standby = 2 => "Standby",
    Enabled = 3 => "Enabled",
    Calibrating = 4 => "Calibrating",
});

/// `AudioMode` (UINT16), Smart Control Plus values (`Lgb` enum). Firmware 3.38 starts in `Off`, where the user EQ
/// gains are stored but not applied; the EQ only sounds in `UserEq`. (The old app's map said 3 = FLAT.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioMode {
    /// EQ bypassed: the headset's own tuning (Smart Control Plus "Neutral").
    Off,
    UserEq,
    Podcast,
    PersonalizedSound,
    ParametricEq,
    HearingEnhancement,
    Other(u16),
}

impl AudioMode {
    pub fn from_u16(v: u16) -> Self {
        match v {
            0 => Self::Off,
            1 => Self::UserEq,
            2 => Self::Podcast,
            3 => Self::PersonalizedSound,
            4 => Self::ParametricEq,
            5 => Self::HearingEnhancement,
            v => Self::Other(v),
        }
    }
    pub fn to_u16(self) -> u16 {
        match self {
            Self::Off => 0,
            Self::UserEq => 1,
            Self::Podcast => 2,
            Self::PersonalizedSound => 3,
            Self::ParametricEq => 4,
            Self::HearingEnhancement => 5,
            Self::Other(v) => v,
        }
    }
    pub fn is_podcast(self) -> bool {
        self == Self::Podcast
    }
}

/// Auto power-off choices offered by the app (timer 0), seconds.
pub const AUTO_POWER_OFF_CHOICES: [u16; 4] = [0, 900, 1800, 3600];
pub const TIMER_POWER_OFF: u8 = 0;
pub const TIMER_LIMBO: u8 = 1;

/// ANC "mode" ids used in the ANC mode/state pairs.
pub mod anc_mode {
    pub const ANTI_WIND: u8 = 1;
    /// Reported by the M4 but never used by the official app. Probably anti-pressure (unverified).
    pub const UNKNOWN_2: u8 = 2;
    pub const ADAPTIVE: u8 = 3;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl Version {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl std::str::FromStr for Version {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        let mut it = s
            .trim()
            .split('.')
            .map(|p| p.parse::<u16>().map_err(|_| ()));
        let v = Version::new(
            it.next().ok_or(())??,
            it.next().unwrap_or(Ok(0))?,
            it.next().unwrap_or(Ok(0))?,
        );
        Ok(v)
    }
}
