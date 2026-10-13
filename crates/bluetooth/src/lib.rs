//! Windows Bluetooth discovery and RFCOMM transport.

pub mod discovery;
pub mod rfcomm;

pub use discovery::{DiscoveryEvent, PairedDevice, Watcher, paired_devices};
pub use rfcomm::{LinkError, LinkEvent, RfcommLink};
