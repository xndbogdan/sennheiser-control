//! RFCOMM byte link to a paired device's custom SPP service (WinRT `StreamSocket`).

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, unbounded};
#[cfg(windows)]
use windows::Devices::Bluetooth::Rfcomm::RfcommServiceId;
#[cfg(windows)]
use windows::Devices::Bluetooth::{BluetoothCacheMode, BluetoothDevice};
#[cfg(windows)]
use windows::Networking::Sockets::{SocketProtectionLevel, StreamSocket};
#[cfg(windows)]
use windows::Storage::Streams::{DataReader, DataWriter, InputStreamOptions};
#[cfg(windows)]
use windows::core::{GUID, HSTRING};
#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[cfg(windows)]
    #[error("Windows error: {0}")]
    Windows(#[from] windows::core::Error),
    #[error("the headset does not expose the control service (is it connected?)")]
    ServiceNotFound,
    #[error("connection timed out")]
    Timeout,
    #[error("Bluetooth error: {0}")]
    BlueZ(String),
    #[error("link closed")]
    Closed,
}

#[derive(Debug)]
pub enum LinkEvent {
    Data(Vec<u8>),
    Closed(Option<String>),
}

pub struct RfcommLink {
    #[cfg(windows)]
    socket: StreamSocket,
    #[cfg(windows)]
    writer: Mutex<DataWriter>,
    #[cfg(not(windows))]
    write: Mutex<std::fs::File>,
}

#[cfg(windows)]
const READ_CHUNK: u32 = 1024;

#[cfg(windows)]
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

/// BlueZ transport: resolve the RFCOMM channel with a manual SDP query over
/// L2CAP, then connect directly. (BlueZ's profile/`ConnectProfile` path hangs
/// against this headset, so SDP + direct connect it is.)
#[cfg(not(windows))]
impl RfcommLink {
    /// Resolve `service` on the device via SDP and connect. Blocks; call from a worker thread.
    pub fn connect(
        device_id: &str,
        service: u128,
        timeout: Duration,
    ) -> Result<(Arc<Self>, Receiver<LinkEvent>), LinkError> {
        let addr: bluer::Address = device_id.parse().map_err(|_| LinkError::ServiceNotFound)?;
        let sdp_budget = timeout / 2;
        let channels = super::discovery::rt().block_on(async {
            tokio::time::timeout(sdp_budget, sdp_rfcomm_channels(addr, service))
                .await
                .map_err(|_| LinkError::Timeout)?
        })?;
        if channels.is_empty() {
            return Err(LinkError::ServiceNotFound);
        }
        log::info!("GAIA service candidates on RFCOMM channels {channels:?}");

        // The headset exposes this UUID on two channels; only one runs GAIA
        // (the other drops the link instantly and can wedge its BT stack, so
        // never hammer: one paced try per candidate, first survivor wins).
        let per_try = (timeout - sdp_budget.min(timeout)) / channels.len().max(1) as u32;
        for ch in channels {
            match try_channel(addr, ch, per_try) {
                Ok(link) if link_alive(&link.1, &link.2) => return Ok((link.0, link.1)),
                Ok((link, _, _)) => link.close(),
                Err(e) => log::info!("channel {ch} rejected: {e}"),
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        Err(LinkError::ServiceNotFound)
    }
}

/// One RFCOMM attempt on a single channel: connect, hand the fd to blocking
/// std IO, start the reader. Returns the live link + its event channel.
#[cfg(not(windows))]
fn try_channel(
    addr: bluer::Address,
    channel: u8,
    timeout: Duration,
) -> Result<(Arc<RfcommLink>, Receiver<LinkEvent>, Sender<LinkEvent>), LinkError> {
    use std::os::unix::io::{AsRawFd, FromRawFd};

    let stream = super::discovery::rt().block_on(async {
        let sa = bluer::rfcomm::SocketAddr::new(addr, channel);
        tokio::time::timeout(timeout, bluer::rfcomm::Stream::connect(sa))
            .await
            .map_err(|_| LinkError::Timeout)?
            .map_err(|e| LinkError::BlueZ(e.to_string()))
    })?;

    // hand the fd to blocking std IO so the engine's sync threads can use it directly
    let dup = unsafe { libc::dup(stream.as_raw_fd()) };
    if dup < 0 {
        return Err(LinkError::BlueZ(
            std::io::Error::last_os_error().to_string(),
        ));
    }
    unsafe {
        let flags = libc::fcntl(dup, libc::F_GETFL);
        if flags >= 0 {
            libc::fcntl(dup, libc::F_SETFL, flags & !libc::O_NONBLOCK);
        }
    }
    drop(stream);
    let reader = unsafe { std::fs::File::from_raw_fd(dup) };
    let writer = reader
        .try_clone()
        .map_err(|e| LinkError::BlueZ(e.to_string()))?;
    let (tx, rx) = unbounded();
    let probe_tx = tx.clone();
    let link = Arc::new(RfcommLink {
        write: Mutex::new(writer),
    });
    thread::Builder::new()
        .name("rfcomm-reader".into())
        .spawn(move || read_loop(reader, tx))
        .ok();
    Ok((link, rx, probe_tx))
}

// A just-connected link counts as ours unless the headset already closed it
// (the GAIA UUID is exposed on two channels; the wrong one drops the DLC
// instantly). Data also proves liveness — requeue it so the session keeps it.
#[cfg(not(windows))]
fn link_alive(rx: &Receiver<LinkEvent>, tx: &Sender<LinkEvent>) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_millis(700);
    loop {
        match rx.try_recv() {
            Err(crossbeam_channel::TryRecvError::Empty) => {
                if std::time::Instant::now() >= deadline {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Ok(LinkEvent::Data(d)) => {
                let _ = tx.send(LinkEvent::Data(d));
                return true;
            }
            _ => return false,
        }
    }
}

/// SDP ServiceSearch (0x02) + ServiceAttribute (0x04) for one UUID, returning
/// every RFCOMM channel in its ProtocolDescriptorLists (attr 0x0004), in order.
#[cfg(not(windows))]
async fn sdp_rfcomm_channels(addr: bluer::Address, uuid: u128) -> Result<Vec<u8>, LinkError> {
    let err = |e: std::io::Error| LinkError::BlueZ(e.to_string());
    let sa = bluer::l2cap::SocketAddr {
        addr,
        addr_type: bluer::AddressType::BrEdr,
        psm: 1,
        cid: 0,
    };
    let sock = bluer::l2cap::SeqPacket::connect(sa).await.map_err(err)?;

    // 0x02 ServiceSearch: pattern{UUID128} + max count + empty continuation
    let mut params = vec![0x35, 0x11, 0x1c];
    params.extend_from_slice(&uuid.to_be_bytes());
    params.extend_from_slice(&0x000au16.to_be_bytes());
    params.push(0x00);
    let r = sdp_transact(&sock, 0x02, 1, &params).await?;
    let cur = u16::from_be_bytes([r[7], r[8]]) as usize;
    if cur == 0 {
        return Err(LinkError::ServiceNotFound);
    }
    // 0x04 ServiceAttribute per handle, asking only for attr 0x0004
    let mut channels = Vec::new();
    for i in 0..cur {
        let h = u32::from_be_bytes([r[9 + 4 * i], r[10 + 4 * i], r[11 + 4 * i], r[12 + 4 * i]]);
        let mut params = Vec::new();
        params.extend_from_slice(&h.to_be_bytes());
        params.extend_from_slice(&0xffffu16.to_be_bytes());
        params.extend_from_slice(&[0x35, 0x03, 0x09, 0x00, 0x04]);
        params.push(0x00);
        let r = sdp_transact(&sock, 0x04, 2, &params).await?;
        let len = u16::from_be_bytes([r[5], r[6]]) as usize;
        if let Some(ch) = sdp_find_channel(&r[7..7 + len])
            && !channels.contains(&ch)
        {
            channels.push(ch);
        }
    }
    Ok(channels)
}

#[cfg(not(windows))]
async fn sdp_transact(
    sock: &bluer::l2cap::SeqPacket,
    pdu: u8,
    tid: u16,
    params: &[u8],
) -> Result<Vec<u8>, LinkError> {
    let fail = |m: String| LinkError::BlueZ(m);
    let mut req = vec![pdu];
    req.extend_from_slice(&tid.to_be_bytes());
    req.extend_from_slice(&(params.len() as u16).to_be_bytes());
    req.extend_from_slice(params);
    // the channel needs a beat after connect before the first send lands (ENOTCONN otherwise)
    let start = std::time::Instant::now();
    loop {
        match sock.send(&req).await {
            Ok(_) => break,
            Err(e)
                if e.kind() == std::io::ErrorKind::NotConnected
                    && start.elapsed() < Duration::from_secs(3) =>
            {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            Err(e) => return Err(fail(format!("SDP send: {e}"))),
        }
    }
    let mut buf = vec![0u8; 4096];
    let n = sock
        .recv(&mut buf)
        .await
        .map_err(|e| fail(format!("SDP recv: {e}")))?;
    // ponytail: no continuation paging; one record list always fits here.
    if buf[0] == 0x01 || buf[0] != pdu + 1 {
        return Err(fail(format!(
            "SDP error response: {:02x?}",
            &buf[..n.min(16)]
        )));
    }
    Ok(buf[..n].to_vec())
}

/// Find `19 00 03 08 <channel>` (UUID16 RFCOMM + UINT8 channel) in attr bytes.
#[cfg(not(windows))]
fn sdp_find_channel(data: &[u8]) -> Option<u8> {
    data.windows(5).find_map(|w| {
        if w[0] == 0x19 && w[1] == 0x00 && w[2] == 0x03 && w[3] == 0x08 {
            Some(w[4])
        } else {
            None
        }
    })
}

#[cfg(windows)]
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

#[cfg(not(windows))]
fn read_loop(mut file: std::fs::File, tx: Sender<LinkEvent>) {
    use std::io::Read;
    loop {
        let mut buf = vec![0u8; 1024];
        match file.read(&mut buf) {
            Ok(0) => {
                let _ = tx.send(LinkEvent::Closed(None));
                return;
            }
            Ok(n) => {
                buf.truncate(n);
                if tx.send(LinkEvent::Data(buf)).is_err() {
                    return;
                }
            }
            Err(e) => {
                let _ = tx.send(LinkEvent::Closed(Some(e.to_string())));
                return;
            }
        }
    }
}

impl RfcommLink {
    pub fn send(&self, bytes: &[u8]) -> Result<(), LinkError> {
        #[cfg(windows)]
        {
            let w = self.writer.lock().map_err(|_| LinkError::Closed)?;
            w.WriteBytes(bytes)?;
            w.StoreAsync()?.join()?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            use std::io::Write;
            let mut w = self.write.lock().map_err(|_| LinkError::Closed)?;
            w.write_all(bytes).map_err(|_| LinkError::Closed)?;
            w.flush().map_err(|_| LinkError::Closed)?;
            Ok(())
        }
    }

    pub fn close(&self) {
        #[cfg(windows)]
        let _ = self.socket.Close();
        // shutdown unblocks the reader thread's blocking read; fds close on drop
        #[cfg(not(windows))]
        {
            use std::os::unix::io::AsRawFd;
            let fd = self.write.lock().map(|w| w.as_raw_fd()).unwrap_or(-1);
            if fd >= 0 {
                unsafe {
                    libc::shutdown(fd, libc::SHUT_RDWR);
                }
            }
        }
    }
}

#[cfg(windows)]
impl Drop for RfcommLink {
    fn drop(&mut self) {
        let _ = self.socket.Close();
    }
}
