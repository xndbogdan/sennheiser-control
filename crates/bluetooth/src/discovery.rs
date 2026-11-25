//! Paired Bluetooth Classic devices and their connection state (WinRT `DeviceWatcher`).

#[cfg(windows)]
use crossbeam_channel::{Receiver, Sender, unbounded};
#[cfg(windows)]
use windows::Devices::Bluetooth::{BluetoothAdapter, BluetoothConnectionStatus, BluetoothDevice};
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

/// Paired or connected Classic devices. After an adapter swap Windows can report a connected headset as
/// `IsPaired=false` (seen with a MOMENTUM 4 moved from a MediaTek RZ616 to an Intel radio), so paired alone misses it.
/// `remembered` (a device id) is listed even then, so that headset still shows up while disconnected.
#[cfg(windows)]
fn selector(remembered: Option<&str>) -> windows::core::Result<HSTRING> {
    let mut s = format!(
        "({}) OR ({})",
        BluetoothDevice::GetDeviceSelectorFromPairingState(true)?,
        BluetoothDevice::GetDeviceSelectorFromConnectionStatus(
            BluetoothConnectionStatus::Connected
        )?
    );
    // ids end in the device address, `aa:bb:cc:dd:ee:ff`; checked before it goes into the query
    if let Some(addr) = remembered.and_then(|id| id.rsplit('-').next())
        && addr.len() == 17
        && addr.chars().all(|c| c == ':' || c.is_ascii_hexdigit())
    {
        s += &format!(
            " OR (System.Devices.Aep.ProtocolId:=\"{{E0CBF06C-CD8B-4647-BB8A-263B43F0F974}}\"              AND System.Devices.Aep.DeviceAddress:=\"{addr}\")"
        );
    }
    Ok(HSTRING::from(s))
}

/// Device-id prefix for an adapter: `bluetooth#bluetooth<adapter mac>-`.
#[cfg(windows)]
fn prefix(a: &BluetoothAdapter) -> Option<String> {
    let b = a.BluetoothAddress().ok()?.to_be_bytes();
    let mac = b[2..]
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect::<Vec<_>>()
        .join(":");
    Some(format!("bluetooth#bluetooth{mac}-"))
}

/// Prefixes of every enabled adapter (more than one radio, or after one was swapped out).
#[cfg(windows)]
fn all_adapter_prefixes() -> Vec<String> {
    let Ok(found) = BluetoothAdapter::GetDeviceSelector()
        .and_then(|sel| DeviceInformation::FindAllAsyncAqsFilter(&sel))
        .and_then(|op| op.join())
    else {
        return Vec::new();
    };
    found
        .into_iter()
        .filter_map(|i| {
            prefix(
                &BluetoothAdapter::FromIdAsync(&i.Id().ok()?)
                    .ok()?
                    .join()
                    .ok()?,
            )
        })
        .collect()
}

/// Whether a device sits on an adapter that's still there. Windows keeps pairings of removed adapters, which
/// can't connect. Checks the default adapter first (what nearly everyone has) and only scans all adapters for a
/// device from some other one: it is then either a second live radio (kept) or a removed one (dropped).
/// Keeps the device whenever the adapters can't be read, so a failed query never hides a headset.
#[cfg(windows)]
fn on_live_adapter(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    let default = BluetoothAdapter::GetDefaultAsync()
        .and_then(|op| op.join())
        .ok()
        .and_then(|a| prefix(&a));
    if default.is_some_and(|p| id.starts_with(&p)) {
        return true;
    }
    let all = all_adapter_prefixes();
    all.is_empty() || all.iter().any(|p| id.starts_with(p))
}

#[cfg(windows)]
/// One-shot snapshot of paired (or connected) Bluetooth Classic devices on live adapters.
pub fn paired_devices() -> windows::core::Result<Vec<PairedDevice>> {
    let found = DeviceInformation::FindAllAsyncWithKindAqsFilterAndAdditionalProperties(
        &selector(None)?,
        &additional_properties(),
        DeviceInformationKind::AssociationEndpoint,
    )?
    .join()?;
    found
        .into_iter()
        .map(|d| paired_from_info(&d))
        .filter(|d| d.as_ref().map_or(true, |d| on_live_adapter(&d.id)))
        .collect()
}

/// Live watcher over paired devices. Events arrive on the returned channel; dropping the watcher stops it.
#[cfg(windows)]
pub struct Watcher {
    watcher: DeviceWatcher,
}

#[cfg(windows)]
impl Watcher {
    /// `remembered`: id of the last headset used, listed even when Windows forgot it was paired.
    pub fn start(
        remembered: Option<&str>,
    ) -> windows::core::Result<(Self, Receiver<DiscoveryEvent>)> {
        let (tx, rx) = unbounded();
        let watcher = DeviceInformation::CreateWatcherWithKindAqsFilterAndAdditionalProperties(
            &selector(remembered)?,
            &additional_properties(),
            DeviceInformationKind::AssociationEndpoint,
        )?;
        let t: Sender<DiscoveryEvent> = tx.clone();
        watcher.Added(&TypedEventHandler::new(
            move |_: Ref<DeviceWatcher>, info: Ref<DeviceInformation>| {
                if let Some(info) = info.as_ref()
                    && let Ok(d) = paired_from_info(info)
                    // re-queried per device so an adapter plugged in while running is picked up
                    && on_live_adapter(&d.id)
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

// ----------------------------------------------------------------------------------------------- BlueZ (Linux)

#[cfg(not(windows))]
mod bluez {
    use std::collections::HashMap;
    use std::time::Duration;

    use super::{DiscoveryEvent, PairedDevice};
    use crossbeam_channel::{Receiver, unbounded};

    /// Shared tokio runtime for the blocking BlueZ calls below.
    pub fn rt() -> &'static tokio::runtime::Runtime {
        static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
        RT.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_name("bluetooth-rt")
                .build()
                .expect("tokio runtime")
        })
    }

    pub(crate) async fn adapter(session: &bluer::Session) -> bluer::Result<bluer::Adapter> {
        match session.default_adapter().await {
            Ok(a) => Ok(a),
            Err(_) => {
                let mut names = session.adapter_names().await?;
                names
                    .pop()
                    .ok_or_else(|| bluer::Error {
                        kind: bluer::ErrorKind::NotFound,
                        message: "no Bluetooth adapter".into(),
                    })
                    .and_then(|n| session.adapter(&n))
            }
        }
    }

    async fn snapshot_one(dev: &bluer::Device) -> bluer::Result<Option<PairedDevice>> {
        if !dev.is_paired().await? {
            return Ok(None);
        }
        let address = dev.address().to_string();
        let mut name = dev.alias().await?;
        if name.is_empty() {
            name = dev.name().await?.unwrap_or_default();
        }
        if name.is_empty() {
            name = address.clone();
        }
        Ok(Some(PairedDevice {
            id: address.clone(),
            name,
            address,
            connected: dev.is_connected().await?,
        }))
    }

    async fn snapshot() -> bluer::Result<Vec<PairedDevice>> {
        let session = bluer::Session::new().await?;
        let adapter = adapter(&session).await?;
        let mut out = Vec::new();
        for addr in adapter.device_addresses().await? {
            let dev = adapter.device(addr)?;
            if let Some(d) = snapshot_one(&dev).await? {
                out.push(d);
            }
        }
        Ok(out)
    }

    /// One-shot snapshot of paired devices (same semantics as WinRT's paired selector).
    pub fn paired_devices() -> Result<Vec<PairedDevice>, String> {
        rt().block_on(snapshot()).map_err(|e| e.to_string())
    }

    /// Live watcher, polled: BlueZ has event streams but a 2 s diff loop is
    /// simpler and plenty fast for connect/disconnect tracking.
    /// Dropping the watcher stops its thread.
    pub struct Watcher {
        stop: Arc<std::sync::atomic::AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    use std::sync::Arc;

    impl Watcher {
        /// `_remembered` is a Windows workaround; BlueZ keeps paired state across adapters per device.
        pub fn start(
            _remembered: Option<&str>,
        ) -> Result<(Self, Receiver<DiscoveryEvent>), String> {
            // fail fast when BlueZ/adapter is missing so the engine reports it once
            let first = rt().block_on(snapshot()).map_err(|e| e.to_string())?;
            let (tx, rx) = unbounded();
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let thread = {
                let stop = stop.clone();
                std::thread::Builder::new()
                    .name("bluetooth-watch".into())
                    .spawn(move || {
                        let mut known: HashMap<String, PairedDevice> = HashMap::new();
                        for d in first {
                            let _ = tx.send(DiscoveryEvent::Added(d.clone()));
                            known.insert(d.id.clone(), d);
                        }
                        let _ = tx.send(DiscoveryEvent::EnumerationCompleted);
                        while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                            std::thread::sleep(Duration::from_secs(2));
                            let cur: HashMap<String, PairedDevice> = rt()
                                .block_on(snapshot())
                                .unwrap_or_default()
                                .into_iter()
                                .map(|d| (d.id.clone(), d))
                                .collect();
                            for (id, d) in &cur {
                                match known.get(id) {
                                    None => {
                                        let _ = tx.send(DiscoveryEvent::Added(d.clone()));
                                    }
                                    Some(old) if old != d => {
                                        let _ = tx.send(DiscoveryEvent::Updated {
                                            id: id.clone(),
                                            name: (old.name != d.name).then(|| d.name.clone()),
                                            connected: (old.connected != d.connected)
                                                .then_some(d.connected),
                                        });
                                    }
                                    _ => {}
                                }
                            }
                            for id in known.keys() {
                                if !cur.contains_key(id) {
                                    let _ = tx.send(DiscoveryEvent::Removed { id: id.clone() });
                                }
                            }
                            known = cur;
                        }
                    })
                    .map_err(|e| e.to_string())?
            };
            Ok((
                Self {
                    stop,
                    thread: Some(thread),
                },
                rx,
            ))
        }
    }

    impl Drop for Watcher {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    // ponytail: tests need real BlueZ + paired hardware; the sim covers engine logic.
}

#[cfg(not(windows))]
pub(crate) use bluez::rt;
#[cfg(not(windows))]
pub use bluez::{Watcher, paired_devices};
