//! A simulated MOMENTUM 4 speaking GAIA over an in-memory link. Mirrors behaviour observed on real hardware
//! (fw 3.38.3): responses, set acks, notification echo, state dump on registration, error 5 for invalid values.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use bluetooth::LinkEvent;
use crossbeam_channel::{Receiver, Sender, unbounded};
use parking_lot::Mutex;
use protocol::ids::{VENDOR_QUALCOMM, VENDOR_SENNHEISER, qc, sn};
use protocol::{Deframer, Kind, Packet};

use crate::transport::Transport;

pub const SIM_DEVICE_ID: &str = "sim:momentum4";
pub const SIM_DEVICE_NAME: &str = "MOMENTUM 4";

/// Properties the real headset pushes (on registration and on change). Others only answer gets.
const NOTIFIABLE: &[u16] = &[
    sn::PHYSICAL_STATE_GET,
    sn::CHARGING_STATE_GET,
    sn::BATTERY_LEVEL_GET,
    sn::CODEC_GET,
    sn::SAMPLE_RATE_GET,
    sn::LOW_LATENCY_GET,
    sn::AUDIO_MODE_GET,
    sn::EQ2_SLIDER_GET,
    sn::EQ2_SUBMODE_GET,
    sn::ANC_STATUS_GET,
    sn::ANC_MODE_GET,
    sn::ANC_TRANSPARENCY_GET,
    sn::TOUCH_LOCK_GET,
    sn::PS_MODE_GET,
    sn::PDL_CONNECTION_STATUS_GET,
];

/// Simulated paired-device list: (name, connected). "This PC" is the entry named [`SIM_OWN_HOST`].
const SIM_HOSTS: [(&str, bool); 3] = [("Pixel 9", true), (SIM_OWN_HOST, true), ("iPad", false)];
const SIM_OWN_HOST: &str = "Windows PC (simulated host)";

struct Prop {
    set: Option<u16>,
    value: Vec<u8>,
    /// Allowed values for single-byte setters (inclusive max).
    max: u8,
}

pub struct SimHeadset {
    props: Mutex<BTreeMap<u16, Prop>>,
    sliders: Mutex<[i8; 5]>,
    hosts: Mutex<Vec<(&'static str, bool)>>,
    registered: Mutex<HashSet<u8>>,
    deframer: Mutex<Deframer>,
    tx: Sender<LinkEvent>,
    closed: AtomicBool,
}

fn prop(set: Option<u16>, value: &[u8], max: u8) -> Prop {
    Prop {
        set,
        value: value.to_vec(),
        max,
    }
}

impl SimHeadset {
    fn new(tx: Sender<LinkEvent>) -> Self {
        let mut p = BTreeMap::new();
        p.insert(
            sn::SYSTEM_RELEASE_VERSION_GET,
            prop(None, &[0, 3, 0, 38, 0, 3], 0),
        );
        p.insert(sn::MODEL_ID_GET, prop(None, b"M4AEBT Black\0", 0));
        p.insert(sn::HW_REVISION_GET, prop(None, &[0, 4, 0, 0], 0));
        // HC_SIM_BATTERY lets UI work start from a chosen level (e.g. to see the low-battery alert)
        let battery = std::env::var("HC_SIM_BATTERY")
            .ok()
            .and_then(|v| v.parse::<u8>().ok())
            .unwrap_or(76);
        p.insert(sn::BATTERY_LEVEL_GET, prop(None, &[battery], 0));
        p.insert(sn::CHARGING_STATE_GET, prop(None, &[0, 0], 0));
        p.insert(sn::CODEC_GET, prop(None, &[8], 0));
        p.insert(sn::SAMPLE_RATE_GET, prop(None, &[0, 0, 0xBB, 0x80], 0));
        // HC_SIM_NOISE=custom|off starts in that noise mode instead of adaptive
        let noise = std::env::var("HC_SIM_NOISE").unwrap_or_default();
        let adaptive = u8::from(noise != "custom");
        p.insert(
            sn::ANC_MODE_GET,
            prop(Some(sn::ANC_MODE_SET), &[1, 2, 2, 0, 3, adaptive], 0),
        );
        let anc = u8::from(noise != "off");
        p.insert(
            sn::ANC_STATUS_GET,
            prop(Some(sn::ANC_STATUS_SET), &[anc], 1),
        );
        p.insert(
            sn::ANC_TRANSPARENCY_GET,
            prop(Some(sn::ANC_TRANSPARENCY_SET), &[30], 100),
        );
        p.insert(
            sn::TH_PAUSE_ON_FULL_GET,
            prop(Some(sn::TH_PAUSE_ON_FULL_SET), &[1], 1),
        );
        p.insert(sn::SIDETONE_GET, prop(Some(sn::SIDETONE_SET), &[2], 4));
        p.insert(
            sn::AUDIO_MODE_GET,
            prop(Some(sn::AUDIO_MODE_SET), &[0, 0], 5),
        );
        p.insert(
            sn::PROMPT_MODE_GET,
            prop(Some(sn::PROMPT_MODE_SET), &[2], 2),
        );
        p.insert(
            sn::ON_HEAD_DETECTION_GET,
            prop(Some(sn::ON_HEAD_DETECTION_SET), &[1], 1),
        );
        p.insert(
            sn::SMART_PAUSE_GET,
            prop(Some(sn::SMART_PAUSE_SET), &[1], 1),
        );
        p.insert(sn::AUTO_CALL_GET, prop(Some(sn::AUTO_CALL_SET), &[0], 1));
        p.insert(
            sn::COMFORT_CALL_GET,
            prop(Some(sn::COMFORT_CALL_SET), &[1], 1),
        );
        p.insert(sn::TOUCH_LOCK_GET, prop(Some(sn::TOUCH_LOCK_SET), &[0], 1));
        p.insert(
            sn::BT_COMPAT_MODE_GET,
            prop(Some(sn::BT_COMPAT_MODE_SET), &[0], 1),
        );
        p.insert(
            sn::LOW_LATENCY_GET,
            prop(Some(sn::LOW_LATENCY_SET), &[0], 1),
        );
        p.insert(sn::PHYSICAL_STATE_GET, prop(None, &[3], 0));
        p.insert(sn::EQ2_CONFIG_GET, prop(None, &[5, 0xC4, 0x3C, 0, 0], 0));
        p.insert(
            sn::EQ2_SUBMODE_GET,
            prop(Some(sn::EQ2_SUBMODE_SET), &[0], 1),
        );
        p.insert(sn::PS_MODE_GET, prop(Some(sn::PS_MODE_SET), &[1], 4));
        p.insert(sn::PDL_MAX_CONNECTIONS_GET, prop(None, &[2], 0));
        Self {
            props: Mutex::new(p),
            sliders: Mutex::new([35, 20, -15, 15, 30]),
            hosts: Mutex::new(SIM_HOSTS.to_vec()),
            registered: Mutex::new(HashSet::new()),
            deframer: Mutex::new(Deframer::new()),
            tx,
            closed: AtomicBool::new(false),
        }
    }

    fn out(&self, vendor: u16, id: u16, payload: Vec<u8>) {
        if self.closed.load(Ordering::SeqCst) {
            return;
        }
        let bytes = Packet::new(vendor, id, payload)
            .encode()
            .expect("sim frame");
        let _ = self.tx.send(LinkEvent::Data(bytes));
    }

    fn respond(&self, cmd: u16, payload: Vec<u8>) {
        self.out(VENDOR_SENNHEISER, cmd | 0x0100, payload);
    }

    fn error(&self, vendor: u16, cmd: u16, code: u8) {
        self.out(vendor, cmd | 0x0180, vec![code]);
    }

    fn notify(&self, get_id: u16, payload: Vec<u8>) {
        let feature = (get_id >> 9) as u8;
        if NOTIFIABLE.contains(&get_id) && self.registered.lock().contains(&feature) {
            self.out(VENDOR_SENNHEISER, get_id | 0x0080, payload);
        }
    }

    fn handle(&self, p: Packet) {
        if p.kind() != Kind::Command {
            return;
        }
        let id = p.id;
        let d = p.payload.as_slice();
        if p.vendor == VENDOR_QUALCOMM {
            match id {
                qc::SERIAL_NUMBER_GET => {
                    self.out(VENDOR_QUALCOMM, id | 0x100, b"SIM000000001\0".to_vec())
                }
                qc::API_VERSION_GET => self.out(VENDOR_QUALCOMM, id | 0x100, vec![3, 1]),
                _ => self.error(VENDOR_QUALCOMM, id, 1),
            }
            return;
        }
        match id {
            sn::REGISTER_NOTIFICATION => {
                let f = d.first().copied().unwrap_or(0);
                self.registered.lock().insert(f);
                self.respond(id, vec![]);
                self.dump_feature(f);
            }
            sn::TIMER_GET => match d.first() {
                Some(0) => self.respond(id, vec![0, 0x03, 0x84]),
                _ => self.error(VENDOR_SENNHEISER, id, 5),
            },
            sn::TIMER_SET => self.respond(id, vec![]),
            sn::EQ2_SLIDER_GET => match d
                .first()
                .and_then(|&b| self.sliders.lock().get(b as usize).copied())
            {
                Some(g) => self.respond(id, vec![g as u8]),
                None => self.error(VENDOR_SENNHEISER, id, 5),
            },
            sn::EQ2_SLIDER_SET => {
                let (Some(&b), Some(&g)) = (d.first(), d.get(1)) else {
                    return self.error(VENDOR_SENNHEISER, id, 5);
                };
                let g = g as i8;
                if b > 4 || !(-60..=60).contains(&g) {
                    return self.error(VENDOR_SENNHEISER, id, 5);
                }
                self.sliders.lock()[b as usize] = g;
                self.respond(id, vec![]);
                let all = self.sliders.lock().map(|v| v as u8).to_vec();
                self.notify(sn::EQ2_SLIDER_GET, all);
            }
            sn::PDL_SIZE_GET => {
                let n = self.hosts.lock().len() as u16;
                self.respond(id, n.to_be_bytes().to_vec());
            }
            sn::PDL_OWN_INDEX_GET => {
                let own = self.hosts.lock().iter().position(|h| h.0 == SIM_OWN_HOST);
                self.respond(id, vec![own.unwrap_or(0) as u8]);
            }
            sn::PDL_INFO_GET => {
                let host = d
                    .first()
                    .and_then(|&i| self.hosts.lock().get(i as usize).copied());
                match host {
                    Some((name, connected)) => {
                        let mut v = vec![d[0], d[0], u8::from(connected)];
                        v.extend_from_slice(name.as_bytes());
                        v.push(0);
                        self.respond(id, v);
                    }
                    None => self.error(VENDOR_SENNHEISER, id, 5),
                }
            }
            sn::PDL_CONNECT | sn::PDL_DISCONNECT => {
                let on = id == sn::PDL_CONNECT;
                let i = d.first().map(|&i| i as usize);
                let mut hosts = self.hosts.lock();
                // this PC can't be dropped: it would cut the control link
                match i.and_then(|i| hosts.get_mut(i).map(|h| (i, h))) {
                    Some((i, h)) if h.0 != SIM_OWN_HOST => {
                        h.1 = on;
                        drop(hosts);
                        self.respond(id, vec![]);
                        self.notify(sn::PDL_CONNECTION_STATUS_GET, vec![i as u8, u8::from(on)]);
                    }
                    _ => {
                        drop(hosts);
                        self.error(VENDOR_SENNHEISER, id, 5);
                    }
                }
            }
            sn::PDL_DELETE_ENTRY => {
                let i = d.first().map(|&i| i as usize);
                let mut hosts = self.hosts.lock();
                match i.filter(|&i| hosts.get(i).is_some_and(|h| h.0 != SIM_OWN_HOST)) {
                    Some(i) => {
                        hosts.remove(i);
                        drop(hosts);
                        self.respond(id, vec![i as u8]);
                    }
                    None => {
                        drop(hosts);
                        self.error(VENDOR_SENNHEISER, id, 5);
                    }
                }
            }
            sn::FACTORY_RESET => self.respond(id, vec![]),
            sn::ANC_MODE_SET => {
                let (Some(&mode), Some(&state)) = (d.first(), d.get(1)) else {
                    return self.error(VENDOR_SENNHEISER, id, 5);
                };
                let mut props = self.props.lock();
                let pairs = &mut props.get_mut(&sn::ANC_MODE_GET).unwrap().value;
                match pairs.chunks_exact_mut(2).find(|c| c[0] == mode) {
                    Some(c) => c[1] = state,
                    None => {
                        drop(props);
                        return self.error(VENDOR_SENNHEISER, id, 5);
                    }
                }
                let v = pairs.clone();
                drop(props);
                self.respond(id, vec![]);
                self.notify(sn::ANC_MODE_GET, v);
            }
            _ => {
                let mut props = self.props.lock();
                if let Some(pr) = props.get(&id) {
                    let v = pr.value.clone();
                    drop(props);
                    return self.respond(id, v);
                }
                let Some((&get_id, pr)) = props.iter_mut().find(|(_, pr)| pr.set == Some(id))
                else {
                    drop(props);
                    return self.error(VENDOR_SENNHEISER, id, 1);
                };
                let value = d.to_vec();
                let last = value.last().copied().unwrap_or(0);
                if value.len() != pr.value.len() || last > pr.max {
                    drop(props);
                    return self.error(VENDOR_SENNHEISER, id, 5);
                }
                pr.value = value.clone();
                drop(props);
                self.respond(id, vec![]);
                self.notify(get_id, value);
            }
        }
    }

    fn dump_feature(&self, feature: u8) {
        let snapshot: Vec<(u16, Vec<u8>)> = self
            .props
            .lock()
            .iter()
            .filter(|(id, _)| (**id >> 9) as u8 == feature && NOTIFIABLE.contains(*id))
            .map(|(id, pr)| (*id, pr.value.clone()))
            .collect();
        for (id, v) in snapshot {
            self.notify(id, v);
        }
        if feature == protocol::ids::feature::USER_EQ {
            let all = self.sliders.lock().map(|v| v as u8).to_vec();
            self.notify(sn::EQ2_SLIDER_GET, all);
        }
    }

    fn drain_battery(self: Arc<Self>) {
        while !self.closed.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_secs(20));
            let v = {
                let mut props = self.props.lock();
                let b = &mut props.get_mut(&sn::BATTERY_LEVEL_GET).unwrap().value[0];
                *b = if *b <= 5 { 100 } else { *b - 1 };
                *b
            };
            self.notify(sn::BATTERY_LEVEL_GET, vec![v]);
        }
    }
}

impl Transport for SimHeadset {
    fn send(&self, bytes: &[u8]) -> Result<(), String> {
        if self.closed.load(Ordering::SeqCst) {
            return Err("closed".into());
        }
        let packets = self.deframer.lock().push(bytes);
        for p in packets {
            self.handle(p);
        }
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
    let sim = Arc::new(SimHeadset::new(tx));
    {
        let sim = sim.clone();
        thread::Builder::new()
            .name("sim-battery".into())
            .spawn(move || sim.drain_battery())
            .ok();
    }
    (sim as Arc<dyn Transport>, rx)
}
