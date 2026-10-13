//! RFCOMM byte link to a paired device's custom SPP service (WinRT `StreamSocket`).

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, unbounded};
use windows::Devices::Bluetooth::Rfcomm::RfcommServiceId;
use windows::Devices::Bluetooth::{BluetoothCacheMode, BluetoothDevice};
use windows::Networking::Sockets::{SocketProtectionLevel, StreamSocket};
use windows::Storage::Streams::{DataReader, DataWriter, InputStreamOptions};
use windows::core::{GUID, HSTRING};

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("Windows error: {0}")]
    Windows(#[from] windows::core::Error),
    #[error("the headset does not expose the control service (is it connected?)")]
    ServiceNotFound,
    #[error("connection timed out")]
    Timeout,
    #[error("link closed")]
    Closed,
}

#[derive(Debug)]
pub enum LinkEvent {
    Data(Vec<u8>),
    Closed(Option<String>),
}

pub struct RfcommLink {
    socket: StreamSocket,
    writer: Mutex<DataWriter>,
}

const READ_CHUNK: u32 = 1024;

impl RfcommLink {
    /// Resolve `service` on the device via SDP and connect. Blocks; call from a worker thread.
    pub fn connect(
        device_id: &str,
        service: u128,
        timeout: Duration,
    ) -> Result<(Arc<Self>, Receiver<LinkEvent>), LinkError> {
        let device = BluetoothDevice::FromIdAsync(&HSTRING::from(device_id))?.join()?;
        let sid = RfcommServiceId::FromUuid(GUID::from_u128(service))?;
        let mut services = None;
        for mode in [BluetoothCacheMode::Cached, BluetoothCacheMode::Uncached] {
            let res = device
                .GetRfcommServicesForIdWithCacheModeAsync(&sid, mode)?
                .join()?;
            let list = res.Services()?;
            if list.Size()? > 0 {
                services = Some(list.GetAt(0)?);
                break;
            }
        }
        let svc = services.ok_or(LinkError::ServiceNotFound)?;

        let socket = StreamSocket::new()?;
        let op = socket.ConnectWithProtectionLevelAsync(
            &svc.ConnectionHostName()?,
            &svc.ConnectionServiceName()?,
            SocketProtectionLevel::BluetoothEncryptionAllowNullAuthentication,
        )?;
        // Cancel the connect if it hangs (device out of range etc.).
        let (done_tx, done_rx) = crossbeam_channel::bounded::<()>(1);
        let watchdog = {
            let op = op.clone();
            thread::spawn(move || {
                if done_rx.recv_timeout(timeout).is_err() {
                    let _ = op.Cancel();
                    return true;
                }
                false
            })
        };
        let res = op.join();
        let _ = done_tx.send(());
        let timed_out = watchdog.join().unwrap_or(false);
        if let Err(e) = res {
            return Err(if timed_out {
                LinkError::Timeout
            } else {
                e.into()
            });
        }

        let writer = DataWriter::CreateDataWriter(&socket.OutputStream()?)?;
        let reader = DataReader::CreateDataReader(&socket.InputStream()?)?;
        reader.SetInputStreamOptions(InputStreamOptions::Partial)?;

        let (tx, rx) = unbounded();
        let link = Arc::new(Self {
            socket,
            writer: Mutex::new(writer),
        });
        thread::Builder::new()
            .name("rfcomm-reader".into())
            .spawn(move || read_loop(reader, tx))
            .ok();
        Ok((link, rx))
    }
}

fn read_loop(reader: DataReader, tx: Sender<LinkEvent>) {
    loop {
        let n = match reader.LoadAsync(READ_CHUNK).and_then(|op| op.join()) {
            Ok(n) => n,
            Err(e) => {
                let _ = tx.send(LinkEvent::Closed(Some(e.message().to_string())));
                return;
            }
        };
        if n == 0 {
            let _ = tx.send(LinkEvent::Closed(None));
            return;
        }
        let mut buf = vec![0u8; n as usize];
        if let Err(e) = reader.ReadBytes(&mut buf) {
            let _ = tx.send(LinkEvent::Closed(Some(e.message().to_string())));
            return;
        }
        if tx.send(LinkEvent::Data(buf)).is_err() {
            return;
        }
    }
}

impl RfcommLink {
    pub fn send(&self, bytes: &[u8]) -> Result<(), LinkError> {
        let w = self.writer.lock().map_err(|_| LinkError::Closed)?;
        w.WriteBytes(bytes)?;
        w.StoreAsync()?.join()?;
        Ok(())
    }

    pub fn close(&self) {
        let _ = self.socket.Close();
    }
}

impl Drop for RfcommLink {
    fn drop(&mut self) {
        let _ = self.socket.Close();
    }
}
