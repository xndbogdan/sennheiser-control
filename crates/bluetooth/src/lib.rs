//! Paired Bluetooth Classic discovery and RFCOMM transport (WinRT on Windows, BlueZ on Linux).

pub mod discovery;
pub mod rfcomm;

pub use discovery::{DiscoveryEvent, PairedDevice, Watcher, paired_devices};
pub use rfcomm::{LinkError, LinkEvent, RfcommLink};
