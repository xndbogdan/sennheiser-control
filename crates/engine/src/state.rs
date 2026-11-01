//! Observable application state.

pub use headset::{Action, EqState, HeadsetState, NoiseMode, PairedHost};
use protocol::Product;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LinkState {
    /// No supported headset connected to Windows.
    #[default]
    Idle,
    Connecting,
    Connected,
    /// Last attempt failed; retrying.
    Retrying(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceEntry {
    pub id: String,
    pub name: String,
    /// From the Bluetooth name; the session refines it from the model id.
    pub product: Product,
    pub address: String,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppState {
    /// Supported headsets (see `protocol::Product`) paired with Windows.
    pub devices: Vec<DeviceEntry>,
    pub active: Option<String>,
    pub link: LinkState,
    pub headset: Option<HeadsetState>,
    pub discovery_done: bool,
    pub last_error: Option<String>,
    pub simulated: bool,
}

impl AppState {
    pub fn active_device(&self) -> Option<&DeviceEntry> {
        let id = self.active.as_ref()?;
        self.devices.iter().find(|d| &d.id == id)
    }

    pub fn connected_devices(&self) -> impl Iterator<Item = &DeviceEntry> {
        self.devices.iter().filter(|d| d.connected)
    }

    /// Device name, with an address suffix when several paired headsets share the same name.
    pub fn display_name(&self, d: &DeviceEntry) -> String {
        let dup = self.devices.iter().filter(|x| x.name == d.name).count() > 1;
        if dup && d.address.len() >= 5 {
            format!(
                "{} · {}",
                d.name,
                d.address[d.address.len() - 5..].to_uppercase()
            )
        } else {
            d.name.clone()
        }
    }

    /// Headset state, only when the control link is up.
    pub fn live(&self) -> Option<&HeadsetState> {
        (self.link == LinkState::Connected)
            .then_some(self.headset.as_ref())
            .flatten()
    }
}
