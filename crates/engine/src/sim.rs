//! `--sim`: the shared simulated MOMENTUM 4 (`headset::sim`) as a transport, with its battery draining over time.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use bluetooth::LinkEvent;
use crossbeam_channel::{Receiver, Sender, unbounded};
use headset::sim::{SimModel, SimOptions};
use parking_lot::Mutex;

use crate::transport::Transport;

pub const SIM_DEVICE_ID: &str = "sim:momentum4";
pub const SIM_DEVICE_NAME: &str = "MOMENTUM 4";

pub struct SimHeadset {
    model: Mutex<SimModel>,
    tx: Sender<LinkEvent>,
    closed: AtomicBool,
}

/// Starting state from the dev variables `HC_SIM_BATTERY`, `HC_SIM_NOISE=custom|off` and `HC_SIM_EQ=custom`.
fn options() -> SimOptions {
    let var = |k: &str| std::env::var(k).unwrap_or_default();
    let noise = var("HC_SIM_NOISE");
    SimOptions {
        battery: var("HC_SIM_BATTERY")
            .parse()
            .unwrap_or(SimOptions::default().battery),
        adaptive: noise != "custom",
        anc_on: noise != "off",
        custom_eq: var("HC_SIM_EQ") == "custom",
    }
}

impl SimHeadset {
    fn deliver(&self, frames: Vec<Vec<u8>>) {
        if self.closed.load(Ordering::SeqCst) {
            return;
        }
        for f in frames {
            let _ = self.tx.send(LinkEvent::Data(f));
        }
    }

    fn drain_battery(self: Arc<Self>) {
        while !self.closed.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_secs(20));
            let frames = self.model.lock().drain_battery();
            self.deliver(frames);
        }
    }
}

impl Transport for SimHeadset {
    fn send(&self, bytes: &[u8]) -> Result<(), String> {
        if self.closed.load(Ordering::SeqCst) {
            return Err("closed".into());
        }
        let frames = self.model.lock().receive(bytes);
        self.deliver(frames);
        Ok(())
    }

    fn close(&self) {
        if !self.closed.swap(true, Ordering::SeqCst) {
            let _ = self.tx.send(LinkEvent::Closed(None));
        }
    }
}

/// Connect to a fresh simulated headset.
pub fn connect() -> (Arc<dyn Transport>, Receiver<LinkEvent>) {
    let (tx, rx) = unbounded();
    let sim = Arc::new(SimHeadset {
        model: Mutex::new(SimModel::new(options())),
        tx,
        closed: AtomicBool::new(false),
    });
    {
        let sim = sim.clone();
        thread::Builder::new()
            .name("sim-battery".into())
            .spawn(move || sim.drain_battery())
            .ok();
    }
    (sim as Arc<dyn Transport>, rx)
}
