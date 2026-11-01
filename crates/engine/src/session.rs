//! One control session with one headset: connect, initial sync, then execute jobs until the link drops.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, select, unbounded};
use parking_lot::Mutex;
use protocol::ids::feature;
use protocol::{Capabilities, EqGen, NoiseStyle, Product, Request, Update, decode};

use crate::client::{Client, ClientError, Unsolicited};
use crate::transport::Transport;

#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// Link up and identity read; the rest of the state streams in.
    Connected,
    Updates(Vec<Update>),
    CommandFailed(String),
    /// Could not connect / initialise.
    Failed(String),
    /// Link closed after having been up.
    Ended(Option<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    High,
    Low,
}

pub type Connector = Arc<
    dyn Fn(&str) -> Result<(Arc<dyn Transport>, Receiver<bluetooth::LinkEvent>), String>
        + Send
        + Sync,
>;

pub struct SessionHandle {
    pub generation: u64,
    pub device_id: String,
    high: Sender<Vec<Request>>,
    low: Sender<Vec<Request>>,
    stop: Arc<AtomicBool>,
    transport: Arc<Mutex<Option<Arc<dyn Transport>>>>,
}

impl SessionHandle {
    pub fn submit(&self, reqs: Vec<Request>, priority: Priority) {
        if reqs.is_empty() {
            return;
        }
        let _ = match priority {
            Priority::High => self.high.send(reqs),
            Priority::Low => self.low.send(reqs),
        };
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.transport.lock().take() {
            t.close();
        }
    }
}

impl Drop for SessionHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn spawn(
    generation: u64,
    device_id: String,
    product: Product,
    connector: Connector,
    events: Sender<(u64, SessionEvent)>,
) -> SessionHandle {
    let (high_tx, high_rx) = unbounded();
    let (low_tx, low_rx) = unbounded();
    let stop = Arc::new(AtomicBool::new(false));
    let transport = Arc::new(Mutex::new(None));
    let handle = SessionHandle {
        generation,
        device_id: device_id.clone(),
        high: high_tx,
        low: low_tx,
        stop: stop.clone(),
        transport: transport.clone(),
    };
    thread::Builder::new()
        .name(format!("session-{generation}"))
        .spawn(move || {
            let w = Worker {
                generation,
                events,
                stop,
                high: high_rx,
                low: low_rx,
                product,
                caps: Capabilities::default(),
            };
            w.run(&device_id, connector, transport);
        })
        .expect("spawn session");
    handle
}

struct Worker {
    generation: u64,
    events: Sender<(u64, SessionEvent)>,
    stop: Arc<AtomicBool>,
    high: Receiver<Vec<Request>>,
    low: Receiver<Vec<Request>>,
    product: Product,
    caps: Capabilities,
}

const BATTERY_POLL: Duration = Duration::from_secs(300);
const MAX_CONSECUTIVE_TIMEOUTS: u32 = 3;

impl Worker {
    fn emit(&self, ev: SessionEvent) {
        let _ = self.events.send((self.generation, ev));
    }

    fn run(
        mut self,
        device_id: &str,
        connector: Connector,
        transport_slot: Arc<Mutex<Option<Arc<dyn Transport>>>>,
    ) {
        let (transport, link_rx) = match connector(device_id) {
            Ok(t) => t,
            Err(e) => return self.emit(SessionEvent::Failed(e)),
        };
        if self.stop.load(Ordering::SeqCst) {
            transport.close();
            return;
        }
        *transport_slot.lock() = Some(transport.clone());
        let (unsol_tx, unsol_rx) = unbounded();
        let client = Client::new(transport, link_rx, unsol_tx);
        self.spawn_forwarder(unsol_rx);

        if let Err(e) = self.init(&client) {
            client.close();
            return self.emit(SessionEvent::Failed(e));
        }
        self.emit(SessionEvent::Connected);
        let refresh = self.refresh_requests();
        let reason = match self.execute(&client, &refresh) {
            Err(reason) => reason,
            Ok(()) => self.job_loop(&client),
        };
        client.close();
        if !self.stop.load(Ordering::SeqCst) {
            self.emit(SessionEvent::Ended(reason));
        }
    }

    fn spawn_forwarder(&self, rx: Receiver<Unsolicited>) {
        let events = self.events.clone();
        let generation = self.generation;
        thread::Builder::new()
            .name(format!("session-{generation}-ntf"))
            .spawn(move || {
                for u in rx.iter() {
                    match u {
                        Unsolicited::Packet(p) => {
                            let ups = decode(&p, None);
                            if !ups.is_empty() {
                                let _ = events.send((generation, SessionEvent::Updates(ups)));
                            }
                        }
                        Unsolicited::Closed(_) => return,
                    }
                }
            })
            .ok();
    }

    fn init(&mut self, client: &Client) -> Result<(), String> {
        let fw = client
            .request(&Request::FirmwareVersion)
            .map_err(|e| format!("reading firmware version: {e}"))?;
        let mut ups = decode(&fw, Some(&Request::FirmwareVersion));
        for req in [Request::ModelId, Request::SerialNumber, Request::HwRevision] {
            if let Ok(p) = client.request(&req) {
                ups.extend(decode(&p, Some(&req)));
            }
        }
        // the model id is more reliable than the (renamable) Bluetooth name
        let model_id = ups.iter().find_map(|u| match u {
            Update::ModelId(m) => Some(m.clone()),
            _ => None,
        });
        if let Some((p, _)) = model_id.as_deref().and_then(Product::from_model_id) {
            self.product = p;
        }
        let firmware = ups.iter().find_map(|u| match u {
            Update::FirmwareVersion(v) => Some(*v),
            _ => None,
        });
        if let Some(v) = firmware {
            self.caps = Capabilities::new(self.product, v);
        }
        let identity = format!(
            "{} (model id {:?}, firmware {})",
            self.product.name(),
            model_id.as_deref().unwrap_or("?"),
            firmware
                .map(|v| v.to_string())
                .unwrap_or_else(|| "?".into())
        );
        if self.product.verified() {
            log::info!("connected: {identity}");
        } else {
            // lands in the error log, so reports from untested models say what was connected
            log::warn!("connected to a model not yet tested with this app: {identity}");
        }
        ups.insert(0, Update::Product(self.product));
        self.emit(SessionEvent::Updates(ups));
        Ok(())
    }

    fn refresh_requests(&self) -> Vec<Request> {
        let c = &self.caps;
        let mut r = vec![];
        let mut features = vec![
            feature::DEVICE,
            feature::BATTERY,
            feature::GENERIC_AUDIO,
            feature::DEVICE_MANAGEMENT,
        ];
        features.extend([feature::MMI, feature::TRANSPARENT_HEARING, feature::ANC]);
        if c.eq == EqGen::Gen2 {
            features.push(feature::USER_EQ);
        }
        if c.sound_personalization {
            features.push(feature::PERSONALIZED_SOUND);
        }
        r.extend(features.into_iter().map(Request::RegisterNotifications));
        r.extend([Request::Battery, Request::Charging, Request::Codec]);
        r.extend(noise_reads(c));
        r.push(Request::AudioMode);
        if c.sidetone_max > 0 {
            r.push(Request::Sidetone);
        }
        if c.comfort_call {
            r.push(Request::ComfortCall);
        }
        if c.voice_prompts {
            r.push(Request::PromptMode);
        }
        if c.on_head_detection {
            r.push(Request::OnHeadDetection);
        }
        if c.smart_pause {
            r.push(Request::SmartPause);
        }
        if c.auto_power_off {
            r.push(Request::Timer(0));
        }
        if c.auto_answer_calls {
            r.push(Request::AutoCall);
        }
        if c.touch_lock {
            r.push(Request::TouchLock);
        }
        if c.hi_res_audio {
            r.push(Request::BtMode);
        }
        if c.physical_state {
            r.push(Request::PhysicalState);
        }
        if c.sound_personalization {
            r.push(Request::PersonalizedSound);
        }
        match c.eq {
            EqGen::Gen2 => {
                r.push(Request::Eq2Config);
                r.extend((0..protocol::eq::BANDS as u8).map(Request::Eq2Slider));
                r.push(Request::Eq2SubMode);
            }
            EqGen::Gen1 => {
                r.extend([
                    Request::Eq1LevelShift,
                    Request::Eq1Knob(protocol::eq::KNOB_Y_INDEX),
                    Request::Eq1Params(protocol::eq::gen1_gain_query()),
                ]);
            }
            EqGen::Unsupported => {}
        }
        r.extend([
            Request::PairedOwnIndex,
            Request::PairedMaxConnections,
            Request::PairedCount,
        ]);
        r
    }

    /// Execute requests in order. Returns Err(reason) when the link is gone.
    fn execute(&self, client: &Client, reqs: &[Request]) -> Result<(), Option<String>> {
        let mut timeouts = 0;
        let mut batch = Vec::new();
        for req in reqs {
            if self.stop.load(Ordering::SeqCst) {
                return Err(None);
            }
            match client.request(req) {
                Ok(p) => {
                    timeouts = 0;
                    let ups = decode(&p, Some(req));
                    let acked = ups.iter().any(|u| matches!(u, Update::Ack(_)));
                    // follow-ups the response makes possible
                    if let Some(Update::PairedCount(n)) = ups.first() {
                        for i in 0..(*n).min(8) as u8 {
                            if let Ok(p) = client.request(&Request::PairedInfo(i)) {
                                batch.extend(decode(&p, Some(&Request::PairedInfo(i))));
                            }
                        }
                    }
                    batch.extend(ups);
                    if acked {
                        batch.extend(req.implied_updates());
                    }
                }
                Err(ClientError::Closed) => {
                    self.flush(&mut batch);
                    return Err(Some("link closed".into()));
                }
                Err(ClientError::Timeout(t)) => {
                    timeouts += 1;
                    log::warn!("timeout ({t:?}) on {req:?}");
                    if timeouts >= MAX_CONSECUTIVE_TIMEOUTS {
                        self.flush(&mut batch);
                        return Err(Some("headset stopped responding".into()));
                    }
                }
                Err(e @ ClientError::Rejected { .. }) => {
                    if self.product.verified() {
                        log::info!("{req:?}: {e}");
                    } else {
                        log::warn!("{}: {req:?}: {e}", self.product.name());
                    }
                    if is_user_change(req) {
                        self.emit(SessionEvent::CommandFailed(format!(
                            "The headset rejected this change ({e})"
                        )));
                    }
                }
                Err(e) => {
                    log::warn!("{req:?}: {e}");
                    if is_user_change(req) {
                        self.emit(SessionEvent::CommandFailed(e.to_string()));
                    }
                }
            }
        }
        self.flush(&mut batch);
        Ok(())
    }

    fn flush(&self, batch: &mut Vec<Update>) {
        if !batch.is_empty() {
            self.emit(SessionEvent::Updates(std::mem::take(batch)));
        }
    }

    fn job_loop(&self, client: &Client) -> Option<String> {
        let mut next_poll = Instant::now() + BATTERY_POLL;
        loop {
            if self.stop.load(Ordering::SeqCst) || client.is_closed() {
                return Some("link closed".into());
            }
            let job = if let Ok(j) = self.high.try_recv() {
                Some(coalesce(j, &self.high))
            } else if let Ok(j) = self.low.try_recv() {
                Some(j)
            } else {
                select! {
                    recv(self.high) -> j => j.ok().map(|j| coalesce(j, &self.high)),
                    recv(self.low) -> j => j.ok(),
                    default(Duration::from_millis(250)) => None,
                }
            };
            if let Some(reqs) = job
                && let Err(reason) = self.execute(client, &reqs)
            {
                return reason;
            }
            if Instant::now() >= next_poll {
                next_poll = Instant::now() + BATTERY_POLL;
                if let Err(reason) = self.execute(client, &[Request::Battery, Request::Charging]) {
                    return reason;
                }
            }
        }
    }
}

/// Reads behind the noise-control UI of a product (only what its tile uses, so an untested model's error log only
/// shows unexpected rejections).
pub(crate) fn noise_reads(c: &Capabilities) -> Vec<Request> {
    use Request as R;
    let mut r = match c.noise {
        NoiseStyle::None => vec![],
        NoiseStyle::AdaptiveBalance => vec![R::AncModes, R::AncStatus, R::Transparency],
        NoiseStyle::AncOrTransparency => vec![R::TransparentHearingStatus],
        NoiseStyle::AncTile => vec![R::AncStatus, R::AncModes],
        NoiseStyle::SportTile => vec![R::AncModes, R::TransparentHearingStatus],
    };
    if c.anti_wind && !r.contains(&R::AncModes) {
        r.push(R::AncModes);
    }
    if c.transparency_switch {
        r.push(R::TransparentHearingStatus);
    }
    r
}

fn is_user_change(r: &Request) -> bool {
    matches!(
        r,
        Request::PairedConnect(_)
            | Request::PairedDisconnect(_)
            | Request::PairedDeleteEntry(_)
            | Request::FactoryReset
    ) || format!("{r:?}").starts_with("Set")
}

/// Merge all queued high-priority jobs, keeping only the latest value for repeatable setters
/// (slider drags produce many intermediate values).
fn coalesce(first: Vec<Request>, rx: &Receiver<Vec<Request>>) -> Vec<Request> {
    let mut all = first;
    while let Ok(more) = rx.try_recv() {
        all.extend(more);
    }
    let key = |r: &Request| -> Option<(u16, u8)> {
        let id = r.packet().id;
        match r {
            Request::SetEq2Slider { band, .. } => Some((id, *band)),
            Request::SetAncMode { mode, .. } => Some((id, *mode)),
            Request::SetTransparency(_)
            | Request::SetSidetone(_)
            | Request::SetEq2SubMode(_)
            | Request::SetEq1Params(_)
            | Request::SetEq1LevelShift(_)
            | Request::SetEq1Knob { .. }
            | Request::SetAudioMode(_) => Some((id, 0)),
            _ => None,
        }
    };
    let mut out: Vec<Request> = Vec::with_capacity(all.len());
    for r in all {
        if let Some(k) = key(&r)
            && let Some(pos) = out.iter().position(|o| key(o) == Some(k))
        {
            out.remove(pos);
        }
        out.push(r);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesce_keeps_latest_per_key() {
        let (tx, rx) = unbounded();
        tx.send(vec![
            Request::SetTransparency(20),
            Request::SetEq2Slider {
                band: 1,
                gain_x10: 5,
            },
        ])
        .unwrap();
        tx.send(vec![
            Request::SetTransparency(40),
            Request::SetEq2Slider {
                band: 2,
                gain_x10: 7,
            },
        ])
        .unwrap();
        tx.send(vec![
            Request::SetEq2Slider {
                band: 1,
                gain_x10: 9,
            },
            Request::SetSmartPause(true),
        ])
        .unwrap();
        let out = coalesce(vec![Request::SetTransparency(10)], &rx);
        assert_eq!(
            out,
            vec![
                Request::SetTransparency(40),
                Request::SetEq2Slider {
                    band: 2,
                    gain_x10: 7
                },
                Request::SetEq2Slider {
                    band: 1,
                    gain_x10: 9
                },
                Request::SetSmartPause(true),
            ]
        );
    }

    #[test]
    fn user_change_detection() {
        assert!(is_user_change(&Request::SetTransparency(1)));
        assert!(!is_user_change(&Request::Battery));
        assert!(!is_user_change(&Request::Timer(0)));
    }
}
