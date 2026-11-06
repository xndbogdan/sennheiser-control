//! Paired Bluetooth Classic devices and their connection state (WinRT `DeviceWatcher`).

#[cfg(not(windows))]
use crossbeam_channel::Receiver;
#[cfg(windows)]
use crossbeam_channel::{Receiver, Sender, unbounded};
#[cfg(windows)]
use windows::Devices::Bluetooth::BluetoothDevice;
#[cfg(windows)]
use windows::Devices::Enumeration::{
    DeviceInformation, DeviceInformationKind, DeviceInformationUpdate, DeviceWatcher,
};
#[cfg(windows)]
use windows::Foundation::{IPropertyValue, TypedEventHandler};
#[cfg(windows)]
use windows::core::{HSTRING, IInspectable, Interface, Ref};
#[cfg(windows)]
use windows_collections::{IIterable, IMapView};
#[cfg(windows)]
const PROP_CONNECTED: &str = "System.Devices.Aep.IsConnected";
#[cfg(windows)]
const PROP_ADDRESS: &str = "System.Devices.Aep.DeviceAddress";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedDevice {
    /// WinRT device id, usable with `BluetoothDevice::FromIdAsync`.
    pub id: String,
    pub name: String,
    pub address: String,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryEvent {
    Added(PairedDevice),
    Updated {
        id: String,
        name: Option<String>,
        connected: Option<bool>,
    },
    Removed {
        id: String,
    },
    EnumerationCompleted,
}

#[cfg(windows)]
fn prop_bool(props: &IMapView<HSTRING, IInspectable>, key: &str) -> Option<bool> {
    let v = props.Lookup(&HSTRING::from(key)).ok()?;
    v.cast::<IPropertyValue>().ok()?.GetBoolean().ok()
}

#[cfg(windows)]
fn prop_string(props: &IMapView<HSTRING, IInspectable>, key: &str) -> Option<String> {
    let v = props.Lookup(&HSTRING::from(key)).ok()?;
    v.cast::<IPropertyValue>()
        .ok()?
        .GetString()
        .ok()
        .map(|s| s.to_string())
}

#[cfg(windows)]
fn paired_from_info(info: &DeviceInformation) -> windows::core::Result<PairedDevice> {
    let props = info.Properties()?;
    Ok(PairedDevice {
        id: info.Id()?.to_string(),
        name: info.Name()?.to_string(),
        address: prop_string(&props, PROP_ADDRESS).unwrap_or_default(),
        connected: prop_bool(&props, PROP_CONNECTED).unwrap_or(false),
    })
}

#[cfg(windows)]
fn additional_properties() -> IIterable<HSTRING> {
    IIterable::from(vec![
        HSTRING::from(PROP_CONNECTED),
        HSTRING::from(PROP_ADDRESS),
    ])
}

#[cfg(windows)]
/// One-shot snapshot of paired Bluetooth Classic devices.
pub fn paired_devices() -> windows::core::Result<Vec<PairedDevice>> {
    let selector = BluetoothDevice::GetDeviceSelectorFromPairingState(true)?;
    let found = DeviceInformation::FindAllAsyncWithKindAqsFilterAndAdditionalProperties(
        &selector,
        &additional_properties(),
        DeviceInformationKind::AssociationEndpoint,
    )?
    .join()?;
    found.into_iter().map(|d| paired_from_info(&d)).collect()
}

/// Live watcher over paired devices. Events arrive on the returned channel; dropping the watcher stops it.
#[cfg(windows)]
pub struct Watcher {
    watcher: DeviceWatcher,
}

#[cfg(windows)]
impl Watcher {
    pub fn start() -> windows::core::Result<(Self, Receiver<DiscoveryEvent>)> {
        let (tx, rx) = unbounded();
        let selector = BluetoothDevice::GetDeviceSelectorFromPairingState(true)?;
        let watcher = DeviceInformation::CreateWatcherWithKindAqsFilterAndAdditionalProperties(
            &selector,
            &additional_properties(),
            DeviceInformationKind::AssociationEndpoint,
        )?;
        let t: Sender<DiscoveryEvent> = tx.clone();
        watcher.Added(&TypedEventHandler::new(
            move |_: Ref<DeviceWatcher>, info: Ref<DeviceInformation>| {
                if let Some(info) = info.as_ref()
                    && let Ok(d) = paired_from_info(info)
                {
                    let _ = t.send(DiscoveryEvent::Added(d));
                }
                Ok(())
            },
        ))?;
        let t = tx.clone();
        watcher.Updated(&TypedEventHandler::new(
            move |_: Ref<DeviceWatcher>, upd: Ref<DeviceInformationUpdate>| {
                if let Some(upd) = upd.as_ref() {
                    let props = upd.Properties()?;
                    let _ = t.send(DiscoveryEvent::Updated {
                        id: upd.Id()?.to_string(),
                        name: prop_string(&props, "System.ItemNameDisplay"),
                        connected: prop_bool(&props, PROP_CONNECTED),
                    });
                }
                Ok(())
            },
        ))?;
        let t = tx.clone();
        watcher.Removed(&TypedEventHandler::new(
            move |_: Ref<DeviceWatcher>, upd: Ref<DeviceInformationUpdate>| {
                if let Some(upd) = upd.as_ref() {
                    let _ = t.send(DiscoveryEvent::Removed {
                        id: upd.Id()?.to_string(),
                    });
                }
                Ok(())
            },
        ))?;
        let t = tx;
        watcher.EnumerationCompleted(&TypedEventHandler::new(
            move |_: Ref<DeviceWatcher>, _: Ref<IInspectable>| {
                let _ = t.send(DiscoveryEvent::EnumerationCompleted);
                Ok(())
            },
        ))?;
        watcher.Start()?;
        Ok((Self { watcher }, rx))
    }
}

#[cfg(windows)]
impl Drop for Watcher {
    fn drop(&mut self) {
        let _ = self.watcher.Stop();
    }
}

#[cfg(windows)]
#[allow(dead_code)]
fn assert_send() {
    fn is_send<T: Send>() {}
    is_send::<DeviceWatcher>();
    let _ = IInspectable::IID;
}

// ----------------------------------------------------------------------------------------------- non-Windows stubs
// ponytail: BlueZ backend lands next; stubs keep the workspace compiling so `--sim` works everywhere.

/// BlueZ discovery is not implemented yet; use `--sim` or build on Windows.
#[cfg(not(windows))]
pub fn paired_devices() -> Result<Vec<PairedDevice>, String> {
    Err("Bluetooth discovery is Windows-only for now (BlueZ backend TODO)".into())
}

#[cfg(not(windows))]
pub struct Watcher;

#[cfg(not(windows))]
impl Watcher {
    pub fn start() -> Result<(Self, Receiver<DiscoveryEvent>), String> {
        Err("Bluetooth discovery is Windows-only for now (BlueZ backend TODO)".into())
    }
}
