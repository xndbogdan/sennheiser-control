//! WebAssembly bindings for the web app (`web/`): the shared headset core behind a small JS API.
//!
//! The JS side only moves bytes (Web Serial or the simulator), keeps the request queue in order and times requests
//! out. Everything else (encoding, matching responses to requests, decoding, state, per-product features and
//! what each UI action writes) is the same Rust the desktop app runs. Build with `pnpm core` in `web/`.

use std::collections::HashMap;

use headset::sim::{SimModel, SimOptions};
use headset::{Action, HeadsetState, NoiseMode};
use protocol::eq::BANDS;
use protocol::{
    AntiWind, AudioMode, Deframer, EqGen, Kind, NoiseStyle, Product, PromptMode, Request, Update,
    decode,
};
use serde::{Deserialize, Serialize};
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

/// A request ready to write. Hand `token` back to [`Core::begin`] right before writing `bytes`.
#[derive(Debug, Clone, Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct OutRequest {
    pub token: u32,
    #[serde(with = "serde_bytes")]
    #[tsify(type = "Uint8Array")]
    pub bytes: Vec<u8>,
    /// Queued requests with the same key replace each other (slider drags); only the latest is worth sending.
    pub coalesce: Option<String>,
    /// A write the user asked for (a rejection is worth telling them about).
    pub write: bool,
}

#[derive(Debug, Default, Serialize, Tsify)]
pub struct Batch {
    pub requests: Vec<OutRequest>,
}

#[derive(Debug, Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct Rejection {
    pub request: String,
    pub code: u8,
    pub user_change: bool,
}

#[derive(Debug, Default, Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct FeedResult {
    /// The in-flight request this data answered (accepted or rejected).
    pub done: Option<u32>,
    pub rejected: Option<Rejection>,
    /// Reads to queue next (the paired-device entries once their count is known).
    pub follow_ups: Vec<OutRequest>,
    /// The state changed: re-read [`Core::view`].
    pub changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Adaptive,
    Custom,
    Off,
    Anc,
    Outdoor,
    Transparency,
}

impl From<NoiseMode> for Mode {
    fn from(m: NoiseMode) -> Self {
        match m {
            NoiseMode::Adaptive => Mode::Adaptive,
            NoiseMode::Custom => Mode::Custom,
            NoiseMode::Off => Mode::Off,
            NoiseMode::Anc => Mode::Anc,
            NoiseMode::Outdoor => Mode::Outdoor,
            NoiseMode::Transparency => Mode::Transparency,
        }
    }
}

impl From<Mode> for NoiseMode {
    fn from(m: Mode) -> Self {
        match m {
            Mode::Adaptive => NoiseMode::Adaptive,
            Mode::Custom => NoiseMode::Custom,
            Mode::Off => NoiseMode::Off,
            Mode::Anc => NoiseMode::Anc,
            Mode::Outdoor => NoiseMode::Outdoor,
            Mode::Transparency => NoiseMode::Transparency,
        }
    }
}

/// UI actions the web app offers (a subset of [`Action`]).
#[derive(Debug, Clone, Deserialize, Tsify)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum WebAction {
    Noise {
        mode: Mode,
    },
    /// Custom-mode balance: 0 full noise cancelling, 50 neutral, 100 full transparency.
    Balance {
        value: u8,
    },
    /// 0 off, 1 max, 2 auto.
    AntiWind {
        value: u8,
    },
    TransparentHearing {
        on: bool,
    },
    EqBands {
        bands: [f32; BANDS],
    },
    /// `false` = Neutral (EQ off).
    EqEnabled {
        on: bool,
    },
    BassBoost {
        on: bool,
    },
    Podcast {
        on: bool,
    },
    OnHead {
        on: bool,
    },
    SmartPause {
        on: bool,
    },
    AutoCall {
        on: bool,
    },
    ComfortCall {
        on: bool,
    },
    TouchLock {
        on: bool,
    },
    Sidetone {
        value: u8,
    },
    AutoOff {
        seconds: u16,
    },
    /// 0 off, 1 tones only, 2 voice and tones.
    Prompts {
        value: u8,
    },
}

impl WebAction {
    fn action(self) -> Option<Action> {
        Some(match self {
            WebAction::Noise { mode } => Action::SetNoiseMode(mode.into()),
            WebAction::Balance { value } => Action::SetTransparency(value),
            WebAction::AntiWind { value } => Action::SetAntiWind(AntiWind::from_u8(value)?),
            WebAction::TransparentHearing { on } => Action::SetTransparentHearing(on),
            WebAction::EqBands { bands } => Action::SetEqBands(bands.map(protocol::eq::snap)),
            WebAction::EqEnabled { on } => Action::SetEqEnabled(on),
            WebAction::BassBoost { on } => Action::SetBassBoost(on),
            WebAction::Podcast { on } => Action::SetPodcast(on),
            WebAction::OnHead { on } => Action::SetOnHeadDetection(on),
            WebAction::SmartPause { on } => Action::SetSmartPause(on),
            WebAction::AutoCall { on } => Action::SetAutoCall(on),
            WebAction::ComfortCall { on } => Action::SetComfortCall(on),
            WebAction::TouchLock { on } => Action::SetTouchLock(on),
            WebAction::Sidetone { value } => Action::SetSidetone(value),
            WebAction::AutoOff { seconds } => Action::SetAutoOff(seconds),
            WebAction::Prompts { value } => Action::SetPromptMode(PromptMode::from_u8(value)?),
        })
    }
}

#[derive(Debug, Serialize, Tsify)]
pub struct ModeOption {
    pub id: Mode,
    pub label: String,
}

#[derive(Debug, Serialize, Tsify)]
pub struct Preset {
    pub name: String,
    pub bands: [f32; BANDS],
}

#[derive(Debug, Serialize, Tsify)]
pub struct Earbuds {
    pub left: Option<u8>,
    pub right: Option<u8>,
    pub case: Option<u8>,
}

/// Everything the UI shows. `None` means not read yet.
#[derive(Debug, Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct View {
    /// The model id belongs to a product this app supports.
    pub supported: bool,
    /// Checked on real hardware (only the MOMENTUM 4 so far).
    pub verified: bool,
    pub product: String,
    pub model_id: Option<String>,
    pub variant: Option<String>,
    pub firmware: Option<String>,
    pub battery: Option<u8>,
    pub earbuds: Option<Earbuds>,
    pub charging: bool,
    pub codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub noise: NoiseView,
    pub sound: SoundView,
    pub settings: SettingsView,
}

#[derive(Debug, Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct NoiseView {
    pub modes: Vec<ModeOption>,
    pub mode: Option<Mode>,
    /// Custom-mode balance (`ANC_Transparency`).
    pub balance: Option<u8>,
    /// `Some` when the product has a separate wind setting: 0 off, 1 max, 2 auto.
    pub anti_wind: Option<Setting<u8>>,
    /// `Some` when the product has a separate transparent hearing switch.
    pub transparent_hearing: Option<Setting<bool>>,
}

#[derive(Debug, Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct SoundView {
    /// 5-band EQ available on this firmware.
    pub eq: bool,
    /// "Neutral" (EQ off) is a mode this firmware knows.
    pub eq_off: bool,
    /// All five bands have been read.
    pub known: bool,
    pub bands: [f32; BANDS],
    pub min_db: f32,
    pub max_db: f32,
    pub bass_boost: bool,
    /// The user EQ is what's playing (otherwise Neutral or podcast mode).
    pub active: bool,
    pub podcast: Option<bool>,
    pub presets: Vec<Preset>,
    /// "Neutral", a preset name, or "Custom"; `None` in podcast mode.
    pub choice: Option<String>,
}

#[derive(Debug, Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    /// Absent when the product lacks the setting.
    pub on_head: Option<Setting<bool>>,
    pub smart_pause: Option<Setting<bool>>,
    /// Smart pause and auto-answer only work while on-head detection is on.
    pub needs_on_head: bool,
    pub auto_call: Option<Setting<bool>>,
    pub comfort_call: Option<Setting<bool>>,
    pub touch_lock: Option<Setting<bool>>,
    pub sidetone_max: u8,
    pub sidetone: Option<u8>,
    pub auto_off: Option<Setting<u16>>,
    pub prompts: Option<Setting<u8>>,
}

/// A setting the product has; `value` is `None` until it has been read. (Absent when the product lacks it.)
#[derive(Debug, Serialize, Tsify)]
pub struct Setting<T> {
    pub value: Option<T>,
}

fn when<T>(supported: bool, value: Option<T>) -> Option<Setting<T>> {
    supported.then_some(Setting { value })
}

/// The shared core for one connection.
#[wasm_bindgen]
pub struct Core {
    state: HeadsetState,
    deframer: Deframer,
    queued: HashMap<u32, Request>,
    in_flight: Option<(u32, Request)>,
    next_token: u32,
    product_known: bool,
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}

impl Core {
    pub fn new() -> Core {
        Core {
            state: HeadsetState::default(),
            deframer: Deframer::new(),
            queued: HashMap::new(),
            in_flight: None,
            next_token: 1,
            product_known: false,
        }
    }

    fn out(&mut self, r: Request) -> OutRequest {
        let token = self.next_token;
        self.next_token += 1;
        let o = OutRequest {
            token,
            bytes: r.packet().encode().unwrap_or_default(),
            coalesce: headset::coalesce_key(&r).map(|(id, k)| format!("{id}:{k}")),
            write: headset::is_user_change(&r),
        };
        self.queued.insert(token, r);
        o
    }

    fn batch(&mut self, reqs: Vec<Request>) -> Batch {
        Batch {
            requests: reqs.into_iter().map(|r| self.out(r)).collect(),
        }
    }

    /// Firmware version and model id: what identifies the headset. Queue these first.
    pub fn identity_requests(&mut self) -> Batch {
        self.batch(vec![Request::FirmwareVersion, Request::ModelId])
    }

    /// Notification registrations and every read behind the UI. Queue once the identity is known.
    pub fn refresh_requests(&mut self) -> Batch {
        let reqs = headset::refresh_requests(&self.state.caps);
        self.batch(reqs)
    }

    /// Battery level and charging state (Smart Control Plus re-reads them every 60 s).
    pub fn poll_requests(&mut self) -> Batch {
        self.batch(vec![Request::Battery, Request::Charging])
    }

    /// The writes behind a UI action, given the current state.
    pub fn plan(&mut self, action: WebAction) -> Batch {
        if !self.product_known {
            return Batch::default();
        }
        let Some(a) = action.action() else {
            return Batch::default();
        };
        let (reqs, _) = headset::plan(&a, &self.state);
        self.batch(reqs)
    }

    /// The queue dropped this request (a newer write replaced it).
    pub fn drop_request(&mut self, token: u32) {
        self.queued.remove(&token);
    }

    /// The request is about to be written; its response is matched from here on.
    pub fn begin(&mut self, token: u32) {
        if let Some(r) = self.queued.remove(&token) {
            self.in_flight = Some((token, r));
        }
    }

    /// No response in time: stop waiting for it.
    pub fn abandon(&mut self, token: u32) {
        if self.in_flight.as_ref().is_some_and(|(t, _)| *t == token) {
            self.in_flight = None;
        }
    }

    /// Bytes received from the headset.
    pub fn feed(&mut self, bytes: &[u8]) -> FeedResult {
        let mut res = FeedResult::default();
        for p in self.deframer.push(bytes) {
            let reply = matches!(p.kind(), Kind::Response | Kind::Error);
            let matched = reply
                && self.in_flight.as_ref().is_some_and(|(_, r)| {
                    let q = r.packet();
                    q.vendor == p.vendor && q.command_id() == p.command_id()
                });
            if !matched {
                let ups = decode(&p, None);
                res.changed |= self.apply_all(&ups);
                continue;
            }
            let (token, req) = self.in_flight.take().expect("matched in-flight request");
            res.done = Some(token);
            if let Some(code) = p.error_code() {
                res.rejected = Some(Rejection {
                    request: format!("{req:?}"),
                    code,
                    user_change: headset::is_user_change(&req),
                });
                continue;
            }
            let ups = decode(&p, Some(&req));
            for f in ups.iter().flat_map(headset::follow_ups).collect::<Vec<_>>() {
                let o = self.out(f);
                res.follow_ups.push(o);
            }
            res.changed |= self.apply_all(&ups);
            if ups.iter().any(|u| matches!(u, Update::Ack(_))) {
                res.changed |= self.apply_all(&req.implied_updates());
            }
        }
        res
    }

    fn apply_all(&mut self, ups: &[Update]) -> bool {
        let mut changed = false;
        for u in ups {
            changed |= self.state.apply(u);
            // the web has no Bluetooth name to go by: the model id decides the product
            if let Update::ModelId(m) = u
                && let Some((p, _)) = Product::from_model_id(m)
            {
                self.product_known = true;
                changed |= self.state.apply(&Update::Product(p));
            }
        }
        changed
    }

    /// The model id has been read and belongs to a supported product.
    pub fn identified(&self) -> bool {
        self.product_known && self.state.firmware.is_some()
    }

    pub fn view_rs(&self) -> View {
        let h = &self.state;
        let c = &h.caps;
        let eq = c.eq != EqGen::Unsupported;
        let known = h.eq.known;
        let choice = if h.podcast() {
            None
        } else if c.eq_off && !h.eq_active() {
            Some("Neutral".to_string())
        } else {
            let same = |b: &[f32; BANDS]| {
                b.iter()
                    .zip(&h.eq.bands)
                    .all(|(x, y)| (x - y).abs() < 0.051)
            };
            Some(
                h.product
                    .presets()
                    .iter()
                    .find(|(_, b)| same(b))
                    .map_or("Custom", |(n, _)| n)
                    .to_string(),
            )
        };
        let on_head = c.on_head_detection;
        View {
            supported: self.product_known,
            verified: h.product.verified(),
            product: if self.product_known {
                h.product.name().to_string()
            } else {
                h.model_id.clone().unwrap_or_default()
            },
            model_id: h.model_id.clone(),
            variant: h.variant().map(str::to_string),
            firmware: h.firmware.map(|v| v.to_string()),
            battery: h.battery,
            earbuds: h.earbuds.map(|e| Earbuds {
                left: e.left,
                right: e.right,
                case: e.case,
            }),
            charging: h.charging_now(),
            codec: h.codec.map(|c| c.label().to_string()),
            sample_rate: h.sample_rate,
            noise: NoiseView {
                modes: h
                    .noise_modes()
                    .into_iter()
                    .map(|m| ModeOption {
                        id: m.into(),
                        label: m.label().to_string(),
                    })
                    .collect(),
                mode: h.noise_mode().map(Mode::from),
                balance: (c.noise == NoiseStyle::AdaptiveBalance)
                    .then_some(h.transparency)
                    .flatten(),
                anti_wind: when(c.anti_wind, h.anti_wind.map(AntiWind::to_u8)),
                transparent_hearing: when(c.transparency_switch, h.transparent_hearing),
            },
            sound: SoundView {
                eq,
                eq_off: c.eq_off,
                known,
                bands: h.eq.bands,
                min_db: if h.eq.max_db > 0.0 {
                    h.eq.min_db
                } else {
                    protocol::eq::MIN_GAIN_DB
                },
                max_db: if h.eq.max_db > 0.0 {
                    h.eq.max_db
                } else {
                    protocol::eq::MAX_GAIN_DB
                },
                bass_boost: h.eq.bass_boost,
                active: h.eq_active(),
                podcast: h.audio_mode.map(AudioMode::is_podcast),
                presets: h
                    .product
                    .presets()
                    .iter()
                    .map(|(n, b)| Preset {
                        name: n.to_string(),
                        bands: *b,
                    })
                    .collect(),
                choice,
            },
            settings: SettingsView {
                on_head: when(on_head, h.on_head_detection),
                smart_pause: when(c.smart_pause, h.smart_pause),
                needs_on_head: on_head,
                auto_call: when(c.auto_answer_calls, h.auto_call),
                comfort_call: when(c.comfort_call, h.comfort_call),
                touch_lock: when(c.touch_lock, h.touch_lock),
                sidetone_max: c.sidetone_max,
                sidetone: h.sidetone,
                auto_off: when(c.auto_power_off, h.auto_off_seconds),
                prompts: when(c.voice_prompts, h.prompt_mode.map(PromptMode::to_u8)),
            },
        }
    }
}

/// The JS API. Values cross the boundary as plain JS objects typed by the generated `.d.ts`.
#[wasm_bindgen]
impl Core {
    #[wasm_bindgen(constructor)]
    pub fn create() -> Core {
        Core::new()
    }

    /// Firmware version and model id: what identifies the headset. Queue these first.
    #[wasm_bindgen(js_name = identityRequests)]
    pub fn identity_requests_js(&mut self) -> Result<Ts<Batch>, JsError> {
        Ok(self.identity_requests().into_ts()?)
    }

    /// Notification registrations and every read behind the UI. Queue once the identity is known.
    #[wasm_bindgen(js_name = refreshRequests)]
    pub fn refresh_requests_js(&mut self) -> Result<Ts<Batch>, JsError> {
        Ok(self.refresh_requests().into_ts()?)
    }

    /// Battery level and charging state.
    #[wasm_bindgen(js_name = pollRequests)]
    pub fn poll_requests_js(&mut self) -> Result<Ts<Batch>, JsError> {
        Ok(self.poll_requests().into_ts()?)
    }

    /// The writes behind a UI action, given the current state.
    #[wasm_bindgen(js_name = plan)]
    pub fn plan_js(&mut self, action: Ts<WebAction>) -> Result<Ts<Batch>, JsError> {
        let action = action.to_rust()?;
        Ok(self.plan(action).into_ts()?)
    }

    /// The queue dropped this request (a newer write replaced it).
    #[wasm_bindgen(js_name = dropRequest)]
    pub fn drop_request_js(&mut self, token: u32) {
        self.drop_request(token)
    }

    /// The request is about to be written; its response is matched from here on.
    #[wasm_bindgen(js_name = begin)]
    pub fn begin_js(&mut self, token: u32) {
        self.begin(token)
    }

    /// No response in time: stop waiting for it.
    #[wasm_bindgen(js_name = abandon)]
    pub fn abandon_js(&mut self, token: u32) {
        self.abandon(token)
    }

    /// Bytes received from the headset.
    #[wasm_bindgen(js_name = feed)]
    pub fn feed_js(&mut self, bytes: &[u8]) -> Result<Ts<FeedResult>, JsError> {
        Ok(self.feed(bytes).into_ts()?)
    }

    /// The model id has been read and belongs to a supported product.
    #[wasm_bindgen(js_name = identified)]
    pub fn identified_js(&self) -> bool {
        self.identified()
    }

    #[wasm_bindgen(js_name = view)]
    pub fn view_js(&self) -> Result<Ts<View>, JsError> {
        Ok(self.view_rs().into_ts()?)
    }
}

/// The simulated MOMENTUM 4 from the desktop app's `--sim`, for the web demo.
#[wasm_bindgen]
pub struct Sim(SimModel);

impl Default for Sim {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl Sim {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Sim {
        Sim(SimModel::new(SimOptions::default()))
    }

    /// Bytes written to the headset; returns what it sends back.
    pub fn receive(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.0.receive(bytes).concat()
    }

    /// Drain the battery by 1 % (call every few seconds for a lively demo).
    pub fn tick(&mut self) -> Vec<u8> {
        self.0.drain_battery().concat()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run a batch through the simulator the way the web client does: one request at a time.
    fn run(core: &mut Core, sim: &mut Sim, batch: Batch) -> Vec<Rejection> {
        let mut rejected = Vec::new();
        let mut queue: Vec<OutRequest> = batch.requests;
        while !queue.is_empty() {
            let r = queue.remove(0);
            core.begin(r.token);
            let back = sim.receive(&r.bytes);
            let res = core.feed(&back);
            assert_eq!(res.done, Some(r.token), "no answer to {:?}", r.bytes);
            rejected.extend(res.rejected);
            queue.extend(res.follow_ups);
        }
        rejected
    }

    #[test]
    fn connects_to_the_simulator_and_applies_writes() {
        let (mut core, mut sim) = (Core::new(), Sim::new());
        let b = core.identity_requests();
        assert!(run(&mut core, &mut sim, b).is_empty());
        assert!(core.identified());
        let b = core.refresh_requests();
        assert!(run(&mut core, &mut sim, b).is_empty());
        let v = core.view_rs();
        assert!(v.supported && v.verified);
        assert_eq!(v.product, "MOMENTUM 4");
        assert_eq!(v.variant.as_deref(), Some("Black"));
        assert_eq!(v.firmware.as_deref(), Some("3.38.3"));
        assert_eq!(v.battery, Some(76));
        assert_eq!(v.noise.mode, Some(Mode::Adaptive));
        assert_eq!(v.noise.modes.len(), 3);
        assert!(v.sound.eq && v.sound.known);
        assert_eq!(v.sound.choice.as_deref(), Some("Neutral"));
        assert_eq!(v.settings.sidetone, Some(2));

        let b = core.plan(WebAction::Noise { mode: Mode::Custom });
        assert!(run(&mut core, &mut sim, b).is_empty());
        let b = core.plan(WebAction::Balance { value: 80 });
        assert!(run(&mut core, &mut sim, b).is_empty());
        let b = core.plan(WebAction::EqBands {
            bands: [3.5, 2.0, -1.5, 1.5, 3.0],
        });
        assert!(run(&mut core, &mut sim, b).is_empty());
        let v = core.view_rs();
        assert_eq!(v.noise.mode, Some(Mode::Custom));
        assert_eq!(v.noise.balance, Some(80));
        assert_eq!(v.sound.choice.as_deref(), Some("Dance"));
        assert!(v.sound.active);
    }

    #[test]
    fn unknown_model_ids_get_no_writes() {
        let mut core = Core::new();
        core.apply_all(&[
            Update::FirmwareVersion(protocol::Version::new(1, 0, 0)),
            Update::ModelId("HD450 B".into()),
        ]);
        assert!(!core.identified());
        assert!(!core.view_rs().supported);
        assert!(
            core.plan(WebAction::BassBoost { on: true })
                .requests
                .is_empty()
        );
    }

    #[test]
    fn rejected_writes_are_reported() {
        let (mut core, mut sim) = (Core::new(), Sim::new());
        let b = core.identity_requests();
        run(&mut core, &mut sim, b);
        // the simulator only accepts sidetone 0..=4
        let r = core.out(Request::SetSidetone(9));
        core.begin(r.token);
        let res = core.feed(&sim.receive(&r.bytes));
        assert_eq!(res.done, Some(r.token));
        assert!(res.rejected.is_some_and(|r| r.user_change && r.code == 5));
    }
}
