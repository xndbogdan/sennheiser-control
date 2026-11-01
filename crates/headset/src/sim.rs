//! A simulated MOMENTUM 4 speaking GAIA (pure, no threads). The desktop app wraps it as a transport for `--sim`;
//! the web app uses it for its demo mode.

use std::collections::{BTreeMap, HashSet};

use protocol::ids::{VENDOR_QUALCOMM, VENDOR_SENNHEISER, qc, sn};
use protocol::{Deframer, Kind, Packet};

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

/// Starting state of the simulator (the desktop app's `HC_SIM_*` variables).
#[derive(Debug, Clone, Copy)]
pub struct SimOptions {
    pub battery: u8,
    /// Adaptive ANC on (otherwise Custom).
    pub adaptive: bool,
    /// `ANC_Status` on (otherwise noise control is Off).
    pub anc_on: bool,
    /// User EQ on with a curve that matches no preset.
    pub custom_eq: bool,
}

impl Default for SimOptions {
    fn default() -> Self {
        Self {
            battery: 76,
            adaptive: true,
            anc_on: true,
            custom_eq: false,
        }
    }
}

/// A simulated MOMENTUM 4 (fw 3.38.3): GAIA frames in, GAIA frames out. Mirrors behaviour observed on real
/// hardware: sets are acknowledged and echoed as notifications, registration pushes the current values.
pub struct SimModel {
    props: BTreeMap<u16, Prop>,
    sliders: [i8; 5],
    hosts: Vec<(&'static str, bool)>,
    registered: HashSet<u8>,
    deframer: Deframer,
    out: Vec<Vec<u8>>,
}

fn prop(set: Option<u16>, value: &[u8], max: u8) -> Prop {
    Prop {
        set,
        value: value.to_vec(),
        max,
    }
}

impl SimModel {
    pub fn new(o: SimOptions) -> Self {
        let mut p = BTreeMap::new();
        p.insert(
            sn::SYSTEM_RELEASE_VERSION_GET,
            prop(None, &[0, 3, 0, 38, 0, 3], 0),
        );
        p.insert(sn::MODEL_ID_GET, prop(None, b"M4AEBT Black\0", 0));
        p.insert(sn::HW_REVISION_GET, prop(None, &[0, 4, 0, 0], 0));
        // HC_SIM_BATTERY lets UI work start from a chosen level (e.g. to see the low-battery alert)
        let battery = o.battery.min(100);
        p.insert(sn::BATTERY_LEVEL_GET, prop(None, &[battery], 0));
        p.insert(sn::CHARGING_STATE_GET, prop(None, &[0, 0], 0));
        p.insert(sn::CODEC_GET, prop(None, &[8], 0));
        p.insert(sn::SAMPLE_RATE_GET, prop(None, &[0, 0, 0xBB, 0x80], 0));
        // HC_SIM_NOISE=custom|off starts in that noise mode instead of adaptive
        let adaptive = u8::from(o.adaptive);
        p.insert(
            sn::ANC_MODE_GET,
            prop(Some(sn::ANC_MODE_SET), &[1, 2, 2, 0, 3, adaptive], 0),
        );
        let anc = u8::from(o.anc_on);
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
            // HC_SIM_EQ=custom starts with the user EQ on and a curve that matches no preset
            prop(Some(sn::AUDIO_MODE_SET), &[0, u8::from(o.custom_eq)], 5),
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
            props: p,
            sliders: if o.custom_eq {
                [20, 5, -10, 10, 25]
            } else {
                [35, 20, -15, 15, 30]
            },
            hosts: SIM_HOSTS.to_vec(),
            registered: HashSet::new(),
            deframer: Deframer::new(),
            out: Vec::new(),
        }
    }

    /// Feed bytes sent to the headset; returns the frames it sends back.
    pub fn receive(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        for p in self.deframer.push(bytes) {
            self.handle(p);
        }
        std::mem::take(&mut self.out)
    }

    /// One battery tick (the desktop simulator runs it every 20 s): drains 1 %, wrapping to 100 at 5 %.
    pub fn drain_battery(&mut self) -> Vec<Vec<u8>> {
        let b = &mut self.props.get_mut(&sn::BATTERY_LEVEL_GET).unwrap().value[0];
        *b = if *b <= 5 { 100 } else { *b - 1 };
        let v = *b;
        self.notify(sn::BATTERY_LEVEL_GET, vec![v]);
        std::mem::take(&mut self.out)
    }

    fn emit(&mut self, vendor: u16, id: u16, payload: Vec<u8>) {
        let bytes = Packet::new(vendor, id, payload)
            .encode()
            .expect("sim frame");
        self.out.push(bytes);
    }

    fn respond(&mut self, cmd: u16, payload: Vec<u8>) {
        self.emit(VENDOR_SENNHEISER, cmd | 0x0100, payload);
    }

    fn error(&mut self, vendor: u16, cmd: u16, code: u8) {
        self.emit(vendor, cmd | 0x0180, vec![code]);
    }

    fn notify(&mut self, get_id: u16, payload: Vec<u8>) {
        let feature = (get_id >> 9) as u8;
        if NOTIFIABLE.contains(&get_id) && self.registered.contains(&feature) {
            self.emit(VENDOR_SENNHEISER, get_id | 0x0080, payload);
        }
    }

    fn handle(&mut self, p: Packet) {
        if p.kind() != Kind::Command {
            return;
        }
        let id = p.id;
        let d = p.payload.as_slice();
        if p.vendor == VENDOR_QUALCOMM {
            match id {
                qc::SERIAL_NUMBER_GET => {
                    self.emit(VENDOR_QUALCOMM, id | 0x100, b"SIM000000001\0".to_vec())
                }
                qc::API_VERSION_GET => self.emit(VENDOR_QUALCOMM, id | 0x100, vec![3, 1]),
                _ => self.error(VENDOR_QUALCOMM, id, 1),
            }
            return;
        }
        match id {
            sn::REGISTER_NOTIFICATION => {
                let f = d.first().copied().unwrap_or(0);
                self.registered.insert(f);
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
                .and_then(|&b| self.sliders.get(b as usize).copied())
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
                self.sliders[b as usize] = g;
                self.respond(id, vec![]);
                let all = self.sliders.map(|v| v as u8).to_vec();
                self.notify(sn::EQ2_SLIDER_GET, all);
            }
            sn::PDL_SIZE_GET => {
                let n = self.hosts.len() as u16;
                self.respond(id, n.to_be_bytes().to_vec());
            }
            sn::PDL_OWN_INDEX_GET => {
                let own = self.hosts.iter().position(|h| h.0 == SIM_OWN_HOST);
                self.respond(id, vec![own.unwrap_or(0) as u8]);
            }
            sn::PDL_INFO_GET => {
                let host = d.first().and_then(|&i| self.hosts.get(i as usize).copied());
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
                // this PC can't be dropped: it would cut the control link
                match i.and_then(|i| self.hosts.get_mut(i).map(|h| (i, h))) {
                    Some((i, h)) if h.0 != SIM_OWN_HOST => {
                        h.1 = on;
                        self.respond(id, vec![]);
                        self.notify(sn::PDL_CONNECTION_STATUS_GET, vec![i as u8, u8::from(on)]);
                    }
                    _ => {
                        self.error(VENDOR_SENNHEISER, id, 5);
                    }
                }
            }
            sn::PDL_DELETE_ENTRY => {
                let i = d.first().map(|&i| i as usize);
                match i.filter(|&i| self.hosts.get(i).is_some_and(|h| h.0 != SIM_OWN_HOST)) {
                    Some(i) => {
                        self.hosts.remove(i);
                        self.respond(id, vec![i as u8]);
                    }
                    None => {
                        self.error(VENDOR_SENNHEISER, id, 5);
                    }
                }
            }
            sn::FACTORY_RESET => self.respond(id, vec![]),
            sn::ANC_MODE_SET => {
                let (Some(&mode), Some(&state)) = (d.first(), d.get(1)) else {
                    return self.error(VENDOR_SENNHEISER, id, 5);
                };
                let pairs = &mut self.props.get_mut(&sn::ANC_MODE_GET).unwrap().value;
                match pairs.chunks_exact_mut(2).find(|c| c[0] == mode) {
                    Some(c) => c[1] = state,
                    None => {
                        return self.error(VENDOR_SENNHEISER, id, 5);
                    }
                }
                let v = pairs.clone();
                self.respond(id, vec![]);
                self.notify(sn::ANC_MODE_GET, v);
            }
            _ => {
                if let Some(pr) = self.props.get(&id) {
                    let v = pr.value.clone();
                    return self.respond(id, v);
                }
                let Some((&get_id, pr)) = self.props.iter_mut().find(|(_, pr)| pr.set == Some(id))
                else {
                    return self.error(VENDOR_SENNHEISER, id, 1);
                };
                let value = d.to_vec();
                let last = value.last().copied().unwrap_or(0);
                if value.len() != pr.value.len() || last > pr.max {
                    return self.error(VENDOR_SENNHEISER, id, 5);
                }
                pr.value = value.clone();
                self.respond(id, vec![]);
                self.notify(get_id, value);
            }
        }
    }

    fn dump_feature(&mut self, feature: u8) {
        let snapshot: Vec<(u16, Vec<u8>)> = self
            .props
            .iter()
            .filter(|(id, _)| (**id >> 9) as u8 == feature && NOTIFIABLE.contains(*id))
            .map(|(id, pr)| (*id, pr.value.clone()))
            .collect();
        for (id, v) in snapshot {
            self.notify(id, v);
        }
        if feature == protocol::ids::feature::USER_EQ {
            let all = self.sliders.map(|v| v as u8).to_vec();
            self.notify(sn::EQ2_SLIDER_GET, all);
        }
    }
}
