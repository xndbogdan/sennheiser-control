//! Blocking GAIA client: one request in flight at a time, responses matched by (vendor, command id),
//! everything else (notifications, stray responses) forwarded as unsolicited packets.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use bluetooth::LinkEvent;
use crossbeam_channel::{Receiver, Sender, bounded};
use parking_lot::Mutex;
use protocol::{Deframer, Kind, Packet, Request, is_dangerous_packet};

use crate::transport::Transport;

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ClientError {
    #[error("no response within {0:?}")]
    Timeout(Duration),
    #[error("headset rejected command {command:04X} (error {code})")]
    Rejected { command: u16, code: u8 },
    #[error("refusing to send a dangerous command ({0:04X})")]
    Dangerous(u16),
    #[error("link closed")]
    Closed,
    #[error("send failed: {0}")]
    Send(String),
    #[error("encode failed: {0}")]
    Encode(String),
}

/// Unsolicited traffic from the headset.
#[derive(Debug, Clone)]
pub enum Unsolicited {
    Packet(Packet),
    Closed(Option<String>),
}

struct Pending {
    vendor: u16,
    command: u16,
    reply: Sender<Result<Packet, ClientError>>,
}

pub struct Client {
    transport: Arc<dyn Transport>,
    pending: Arc<Mutex<Option<Pending>>>,
    inflight: Mutex<()>,
    allow_dangerous: bool,
    closed: Arc<std::sync::atomic::AtomicBool>,
}

pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(1500);

impl Client {
    /// Spawns the dispatcher thread that consumes `link_rx`. Unsolicited packets go to `unsolicited`.
    pub fn new(
        transport: Arc<dyn Transport>,
        link_rx: Receiver<LinkEvent>,
        unsolicited: Sender<Unsolicited>,
    ) -> Self {
        let pending: Arc<Mutex<Option<Pending>>> = Arc::new(Mutex::new(None));
        let closed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let pending = pending.clone();
            let closed = closed.clone();
            thread::Builder::new()
                .name("gaia-dispatch".into())
                .spawn(move || dispatch(link_rx, pending, unsolicited, closed))
                .expect("spawn dispatcher");
        }
        Self {
            transport,
            pending,
            inflight: Mutex::new(()),
            allow_dangerous: false,
            closed,
        }
    }

    /// Allow deny-listed packets (only for explicit tooling use).
    pub fn allow_dangerous(mut self, allow: bool) -> Self {
        self.allow_dangerous = allow;
        self
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn request(&self, req: &Request) -> Result<Packet, ClientError> {
        if matches!(req, Request::FactoryReset | Request::PairedDeleteEntry(_)) {
            // the only deny-listed commands the app may send, and only as confirmed user actions
            return self.send_unchecked(&req.packet(), DEFAULT_TIMEOUT);
        }
        self.request_timeout(req, DEFAULT_TIMEOUT)
    }

    pub fn request_timeout(&self, req: &Request, timeout: Duration) -> Result<Packet, ClientError> {
        self.send_packet(&req.packet(), timeout)
    }

    pub fn send_packet(&self, packet: &Packet, timeout: Duration) -> Result<Packet, ClientError> {
        if is_dangerous_packet(packet) && !self.allow_dangerous {
            return Err(ClientError::Dangerous(packet.id));
        }
        self.send_unchecked(packet, timeout)
    }

    fn send_unchecked(&self, packet: &Packet, timeout: Duration) -> Result<Packet, ClientError> {
        if self.is_closed() {
            return Err(ClientError::Closed);
        }
        let bytes = packet
            .encode()
            .map_err(|e| ClientError::Encode(e.to_string()))?;
        let _guard = self.inflight.lock();
        let (tx, rx) = bounded(1);
        *self.pending.lock() = Some(Pending {
            vendor: packet.vendor,
            command: packet.command_id(),
            reply: tx,
        });
        log::trace!("-> {}", protocol::hex(&bytes));
        if let Err(e) = self.transport.send(&bytes) {
            self.pending.lock().take();
            return Err(ClientError::Send(e));
        }
        let res = rx
            .recv_timeout(timeout)
            .unwrap_or(Err(ClientError::Timeout(timeout)));
        self.pending.lock().take();
        res
    }

    pub fn close(&self) {
        self.transport.close();
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.transport.close();
    }
}

fn dispatch(
    link_rx: Receiver<LinkEvent>,
    pending: Arc<Mutex<Option<Pending>>>,
    unsolicited: Sender<Unsolicited>,
    closed: Arc<std::sync::atomic::AtomicBool>,
) {
    let mut deframer = Deframer::new();
    for ev in link_rx.iter() {
        match ev {
            LinkEvent::Data(bytes) => {
                log::trace!("<- {}", protocol::hex(&bytes));
                for p in deframer.push(&bytes) {
                    let is_reply = matches!(p.kind(), Kind::Response | Kind::Error);
                    let mut slot = pending.lock();
                    let matches = is_reply
                        && slot
                            .as_ref()
                            .is_some_and(|w| w.vendor == p.vendor && w.command == p.command_id());
                    if matches {
                        let w = slot.take().unwrap();
                        drop(slot);
                        let res = match p.error_code() {
                            Some(code) => Err(ClientError::Rejected {
                                command: p.command_id(),
                                code,
                            }),
                            None => Ok(p),
                        };
                        let _ = w.reply.send(res);
                    } else {
                        drop(slot);
                        let _ = unsolicited.send(Unsolicited::Packet(p));
                    }
                }
            }
            LinkEvent::Closed(reason) => {
                closed.store(true, std::sync::atomic::Ordering::SeqCst);
                if let Some(w) = pending.lock().take() {
                    let _ = w.reply.send(Err(ClientError::Closed));
                }
                let _ = unsolicited.send(Unsolicited::Closed(reason));
                return;
            }
        }
    }
    closed.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = unsolicited.send(Unsolicited::Closed(None));
}
