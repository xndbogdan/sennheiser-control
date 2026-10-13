//! Byte transports the client can run over.

use std::sync::Arc;
use std::time::Duration;

use bluetooth::{LinkEvent, RfcommLink};
use crossbeam_channel::Receiver;

pub trait Transport: Send + Sync {
    fn send(&self, bytes: &[u8]) -> Result<(), String>;
    fn close(&self);
}

impl Transport for RfcommLink {
    fn send(&self, bytes: &[u8]) -> Result<(), String> {
        RfcommLink::send(self, bytes).map_err(|e| e.to_string())
    }
    fn close(&self) {
        RfcommLink::close(self)
    }
}

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Connect to the GAIA3 control service of a paired device.
pub fn connect_rfcomm(
    device_id: &str,
) -> Result<(Arc<dyn Transport>, Receiver<LinkEvent>), String> {
    let (link, rx) = RfcommLink::connect(device_id, protocol::GAIA3_SPP_UUID, CONNECT_TIMEOUT)
        .map_err(|e| e.to_string())?;
    Ok((link as Arc<dyn Transport>, rx))
}
