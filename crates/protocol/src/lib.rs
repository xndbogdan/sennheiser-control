//! GAIA v3 protocol for the Sennheiser MOMENTUM 4 and its GAIA v3 siblings (pure, no OS dependencies).
//! See `PROTOCOL.md`.

pub mod caps;
pub mod eq;
pub mod frame;
pub mod ids;
pub mod product;
pub mod request;
pub mod types;
pub mod update;

pub use caps::{Capabilities, EqGen, NoiseStyle};
pub use frame::{Deframer, FrameError, Kind, Packet, hex};
pub use product::Product;
pub use request::{Request, is_dangerous_packet};
pub use types::*;
pub use update::{Update, decode};

/// RFCOMM service UUID of the GAIA v3 SPP endpoint.
pub const GAIA3_SPP_UUID: u128 = 0xa2129ff3_081b_4c45_8afe_469d9c4842ec;
