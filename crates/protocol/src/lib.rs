//! GAIA v3 protocol for the Sennheiser MOMENTUM 4 (pure, no OS dependencies). See `PROTOCOL.md`.

pub mod caps;
pub mod eq;
pub mod frame;
pub mod ids;
pub mod request;
pub mod types;
pub mod update;

pub use caps::{Capabilities, EqGen};
pub use frame::{Deframer, FrameError, Kind, Packet, hex};
pub use request::{Request, is_dangerous_packet};
pub use types::*;
pub use update::{Update, decode};

/// RFCOMM service UUID of the GAIA v3 SPP endpoint.
pub const GAIA3_SPP_UUID: u128 = 0xa2129ff3_081b_4c45_8afe_469d9c4842ec;

/// Bluetooth names the official app maps to the MOMENTUM 4 ("Mars" is its codename).
pub const M4_BT_NAMES: &[&str] = &["MOMENTUM 4", "Mars"];

pub fn is_m4_name(name: &str) -> bool {
    M4_BT_NAMES
        .iter()
        .any(|n| n.eq_ignore_ascii_case(name.trim()))
}

/// Model id prefix reported by `Versions_ModelId` (e.g. `M4AEBT Denim`).
pub fn m4_variant(model_id: &str) -> Option<&str> {
    let rest = model_id.strip_prefix("M4AEBT")?;
    let v = rest.trim();
    Some(if v.is_empty() { "Standard" } else { v })
}
