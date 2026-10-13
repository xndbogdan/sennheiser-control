//! hsctl — hardware bring-up CLI for the MOMENTUM 4 GAIA protocol.
//!
//!   hsctl scan
//!   hsctl info
//!   hsctl monitor [seconds]
//!   hsctl get <property>
//!   hsctl set <property> <value>
//!   hsctl eq
//!   hsctl raw <hex bytes of vendor+id+payload, e.g. "04 95 12 01"> [--unsafe]
//!
//! Options: --device <name substring>   (default: first connected MOMENTUM 4)
//! Set RUST_LOG=trace to see every frame.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::unbounded;
use engine::{Client, Unsolicited, connect_rfcomm};
use protocol::{Packet, Request, Update, decode, ids::feature};

const PROPS: &[(&str, Request)] = &[
    ("firmware", Request::FirmwareVersion),
    ("model", Request::ModelId),
    ("serial", Request::SerialNumber),
    ("hw", Request::HwRevision),
    ("components", Request::ComponentVersions),
    ("battery", Request::Battery),
    ("charging", Request::Charging),
    ("codec", Request::Codec),
    ("anc", Request::AncModes),
    ("anc-status", Request::AncStatus),
    ("transparency", Request::Transparency),
    ("pause-on-full", Request::PauseOnFullTransparency),
    ("sidetone", Request::Sidetone),
    ("audio-mode", Request::AudioMode),
    ("prompts", Request::PromptMode),
    ("prompt-language", Request::PromptLanguage),
    ("on-head", Request::OnHeadDetection),
    ("smart-pause", Request::SmartPause),
    ("auto-call", Request::AutoCall),
    ("comfort-call", Request::ComfortCall),
    ("touch-lock", Request::TouchLock),
    ("bt-mode", Request::BtMode),
    ("low-latency", Request::LowLatency),
    ("physical", Request::PhysicalState),
    ("auto-off", Request::Timer(0)),
    ("limbo", Request::Timer(1)),
    ("eq-config", Request::Eq2Config),
    ("eq-submode", Request::Eq2SubMode),
    ("personalized", Request::PersonalizedSound),
    ("paired-count", Request::PairedCount),
    ("paired-own", Request::PairedOwnIndex),
    ("paired-max", Request::PairedMaxConnections),
    ("api", Request::ApiVersion),
];

fn parse_bool(v: &str) -> Option<bool> {
    match v {
        "1" | "on" | "true" | "yes" => Some(true),
        "0" | "off" | "false" | "no" => Some(false),
        _ => None,
    }
}

fn set_request(prop: &str, v: &str) -> Option<Request> {
    let n = || v.parse::<i64>().ok();
    Some(match prop {
        "adaptive" => Request::SetAncMode {
            mode: 3,
            state: u8::from(parse_bool(v)?),
        },
        "anti-wind" => Request::SetAncMode {
            mode: 1,
            state: match v {
                "off" => 0,
                "max" => 1,
                "auto" => 2,
                _ => n()? as u8,
            },
        },
        "anc-mode" => {
            let (m, s) = v.split_once(',')?;
            Request::SetAncMode {
                mode: m.parse().ok()?,
                state: s.parse().ok()?,
            }
        }
        "anc-status" => Request::SetAncStatus(parse_bool(v)?),
        "transparency" => Request::SetTransparency(n()?.clamp(0, 100) as u8),
        "pause-on-full" => Request::SetPauseOnFullTransparency(parse_bool(v)?),
        "sidetone" => Request::SetSidetone(n()? as u8),
        "audio-mode" => Request::SetAudioMode(protocol::AudioMode::from_u16(match v {
            "off" | "neutral" => 0,
            "eq" => 1,
            "podcast" => 2,
            _ => n()? as u16,
        })),
        "prompts" => Request::SetPromptMode(protocol::PromptMode::from_u8(n()? as u8)?),
        "on-head" => Request::SetOnHeadDetection(parse_bool(v)?),
        "smart-pause" => Request::SetSmartPause(parse_bool(v)?),
        "auto-call" => Request::SetAutoCall(parse_bool(v)?),
        "comfort-call" => Request::SetComfortCall(parse_bool(v)?),
        "touch-lock" => Request::SetTouchLock(parse_bool(v)?),
        "bt-mode" => Request::SetBtMode(protocol::BtMode::from_u8(n()? as u8)?),
        "auto-off" => Request::SetTimer {
            id: 0,
            seconds: n()? as u16,
        },
        "eq-submode" => Request::SetEq2SubMode(n()? as u8),
        "eq-band" => {
            let (b, g) = v.split_once(',')?;
            Request::SetEq2Slider {
                band: b.parse().ok()?,
                gain_x10: protocol::eq::gen2_units(g.parse().ok()?),
            }
        }
        _ => return None,
    })
}

fn hex_bytes(s: &str) -> Option<Vec<u8>> {
    let clean: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if !clean.len().is_multiple_of(2) {
        return None;
    }
    (0..clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&clean[i..i + 2], 16).ok())
        .collect()
}

struct Session {
    client: Client,
    events: crossbeam_channel::Receiver<Unsolicited>,
}

fn open(device_filter: Option<&str>, allow_dangerous: bool) -> Result<Session, String> {
    let devices = bluetooth::paired_devices().map_err(|e| e.to_string())?;
    let pick = devices
        .iter()
        .filter(|d| match device_filter {
            Some(f) => d.name.to_lowercase().contains(&f.to_lowercase()),
            None => protocol::is_m4_name(&d.name),
        })
        .max_by_key(|d| d.connected)
        .ok_or("no matching paired device")?;
    eprintln!(
        "connecting to {} ({}, connected={}) ...",
        pick.name, pick.address, pick.connected
    );
    let t = Instant::now();
    let (transport, link_rx) = connect_rfcomm(&pick.id)?;
    eprintln!("connected in {:?}", t.elapsed());
    let (tx, rx) = unbounded();
    let client = Client::new(Arc::clone(&transport), link_rx, tx).allow_dangerous(allow_dangerous);
    Ok(Session { client, events: rx })
}

fn show(req: &Request, res: Result<Packet, engine::ClientError>) {
    match res {
        Ok(p) => {
            let ups = decode(&p, Some(req));
            if ups.is_empty() {
                println!("{req:?}: raw {:04X} [{}]", p.id, protocol::hex(&p.payload));
            }
            for u in ups {
                println!("{req:?}: {u:?}");
            }
        }
        Err(e) => println!("{req:?}: ERROR {e}"),
    }
}

fn register_all(client: &Client) {
    for f in [
        feature::DEVICE,
        feature::BATTERY,
        feature::GENERIC_AUDIO,
        feature::USER_EQ,
        feature::DEVICE_MANAGEMENT,
        feature::MMI,
        feature::TRANSPARENT_HEARING,
        feature::ANC,
        feature::PERSONALIZED_SOUND,
    ] {
        if let Err(e) = client.request(&Request::RegisterNotifications(f)) {
            println!("register feature {f}: {e}");
        }
    }
}

fn run() -> Result<(), String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut device = None;
    if let Some(i) = args.iter().position(|a| a == "--device") {
        device = args.get(i + 1).cloned();
        args.drain(i..=(i + 1).min(args.len() - 1));
    }
    let unsafe_ok = args.iter().any(|a| a == "--unsafe");
    args.retain(|a| a != "--unsafe");
    let cmd = args.first().map(String::as_str).unwrap_or("help");

    match cmd {
        "scan" => {
            for d in bluetooth::paired_devices().map_err(|e| e.to_string())? {
                let tag = if protocol::is_m4_name(&d.name) {
                    "  <- MOMENTUM 4"
                } else {
                    ""
                };
                println!(
                    "{:<32} {:<18} connected={:<5}{tag}",
                    d.name, d.address, d.connected
                );
            }
        }
        "info" => {
            let s = open(device.as_deref(), false)?;
            for (_, req) in PROPS {
                show(req, s.client.request(req));
            }
            if let Ok(p) = s.client.request(&Request::Eq2Config)
                && let Some(Update::Eq2Config { bands, .. }) = decode(&p, None).into_iter().next()
            {
                for b in 0..bands {
                    let r = Request::Eq2Slider(b);
                    show(&r, s.client.request(&r));
                }
            }
            if let Ok(p) = s.client.request(&Request::PairedCount)
                && let Some(Update::PairedCount(n)) = decode(&p, None).into_iter().next()
            {
                for i in 0..n.min(8) as u8 {
                    let r = Request::PairedInfo(i);
                    show(&r, s.client.request(&r));
                }
            }
        }
        "get" => {
            let name = args.get(1).ok_or("usage: get <property>")?;
            let req = PROPS
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, r)| r.clone())
                .ok_or("unknown property")?;
            let s = open(device.as_deref(), false)?;
            show(&req, s.client.request(&req));
        }
        "set" => {
            let (name, value) = (
                args.get(1).ok_or("usage: set <prop> <value>")?,
                args.get(2).ok_or("missing value")?,
            );
            let req = set_request(name, value).ok_or("unknown property or bad value")?;
            let s = open(device.as_deref(), false)?;
            register_all(&s.client);
            println!("-> {}", protocol::hex(&req.packet().encode().unwrap()));
            show(&req, s.client.request(&req));
            // give the headset a moment to push notifications triggered by the change
            let end = Instant::now() + Duration::from_millis(1500);
            while let Ok(Unsolicited::Packet(p)) = s.events.recv_deadline(end) {
                println!("  notification {:04X}: {:?}", p.id, decode(&p, None));
            }
        }
        "eq" => {
            let s = open(device.as_deref(), false)?;
            for req in [
                Request::FirmwareVersion,
                Request::Eq2Config,
                Request::Eq2SubMode,
                Request::AudioMode,
            ] {
                show(&req, s.client.request(&req));
            }
            for b in 0..5 {
                let r = Request::Eq2Slider(b);
                show(&r, s.client.request(&r));
            }
        }
        "monitor" => {
            let secs: u64 = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(60);
            let s = open(device.as_deref(), false)?;
            register_all(&s.client);
            println!("monitoring for {secs}s — touch the headset, change modes, take it off...");
            let end = Instant::now() + Duration::from_secs(secs);
            loop {
                match s.events.recv_deadline(end) {
                    Ok(Unsolicited::Packet(p)) => {
                        println!(
                            "{:>7.2}s {:04X}:{:04X} [{}] {:?}",
                            (Instant::now() - (end - Duration::from_secs(secs))).as_secs_f32(),
                            p.vendor,
                            p.id,
                            protocol::hex(&p.payload),
                            decode(&p, None)
                        )
                    }
                    Ok(Unsolicited::Closed(r)) => {
                        println!("link closed: {r:?}");
                        break;
                    }
                    Err(_) => break,
                }
            }
        }
        "engine" => {
            let secs: u64 = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(10);
            let simulate = args.iter().any(|a| a == "--sim");
            let engine = engine::Engine::start(
                engine::EngineOptions {
                    simulate,
                    ..Default::default()
                },
                || {},
            );
            let start = Instant::now();
            let mut last = None;
            while start.elapsed() < Duration::from_secs(secs) {
                let st = engine.snapshot();
                let line = format!(
                    "devices={:?} active={:?} link={:?} battery={:?} charging={:?} mode={:?} transparency={:?} eq={:?} codec={:?} fw={:?}",
                    st.devices
                        .iter()
                        .map(|d| (&d.name, d.connected))
                        .collect::<Vec<_>>(),
                    st.active_device().map(|d| &d.name),
                    st.link,
                    st.live().and_then(|h| h.battery),
                    st.live().and_then(|h| h.charging),
                    st.live().and_then(|h| h.noise_mode()),
                    st.live().and_then(|h| h.transparency),
                    st.live().map(|h| (h.eq.bands, h.eq.bass_boost)),
                    st.live().and_then(|h| h.codec),
                    st.live().and_then(|h| h.firmware),
                );
                if last.as_ref() != Some(&line) {
                    println!("{:>6.2}s {line}", start.elapsed().as_secs_f32());
                    last = Some(line);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            if let Some(h) = engine.snapshot().live() {
                println!("final headset state: {h:#?}");
            }
            engine.shutdown();
        }
        "dump" => {
            // safe surface map: Qualcomm supported features + every Sennheiser feature's notification dump
            let s = open(device.as_deref(), false)?;
            for (name, req) in [("qc api", Request::ApiVersion)] {
                if let Ok(p) = s.client.request(&req) {
                    println!("{name}: [{}]", protocol::hex(&p.payload));
                }
            }
            let mut more = true;
            let mut first = true;
            while more {
                let id = if first {
                    protocol::ids::qc::SUPPORTED_FEATURES_GET
                } else {
                    protocol::ids::qc::SUPPORTED_FEATURES_NEXT_GET
                };
                first = false;
                match s.client.send_packet(
                    &Packet::new(protocol::ids::VENDOR_QUALCOMM, id, vec![]),
                    Duration::from_millis(1500),
                ) {
                    Ok(p) => {
                        more = p.payload.first() == Some(&1);
                        let feats: Vec<String> = p.payload[1..]
                            .chunks(2)
                            .map(|c| format!("{}v{}", c[0], c.get(1).copied().unwrap_or(0)))
                            .collect();
                        println!("qc features: {}", feats.join(" "));
                    }
                    Err(e) => {
                        println!("qc features: {e}");
                        more = false;
                    }
                }
            }
            for f in 0u8..32 {
                match s.client.request(&Request::RegisterNotifications(f)) {
                    Ok(_) => print!("[{f} ok] "),
                    Err(engine::ClientError::Rejected { code, .. }) => print!("[{f} e{code}] "),
                    Err(e) => print!("[{f} {e}] "),
                }
            }
            println!();
            let end = Instant::now() + Duration::from_millis(2500);
            let mut seen = std::collections::BTreeMap::new();
            while let Ok(Unsolicited::Packet(p)) = s.events.recv_deadline(end) {
                seen.insert(p.id, p.payload.clone());
            }
            for (id, payload) in seen {
                println!(
                    "{:04X} (feat {:>2} cmd {:02X}) [{}]",
                    id,
                    id >> 9,
                    id & 0x7F,
                    protocol::hex(&payload)
                );
            }
        }
        "probe-ids" => {
            // probe explicit Sennheiser command ids (hex, comma separated) with an empty payload
            let list = args.get(1).ok_or("usage: probe-ids 2800,2801,...")?;
            let ids: Vec<u16> = list
                .split(',')
                .filter_map(|x| u16::from_str_radix(x.trim().trim_start_matches("0x"), 16).ok())
                .collect();
            let s = open(device.as_deref(), false)?;
            for id in ids {
                let p = Packet::new(protocol::ids::VENDOR_SENNHEISER, id, vec![]);
                if protocol::is_dangerous_packet(&p) {
                    println!("{id:04X}: skipped (deny-listed)");
                    continue;
                }
                match s.client.send_packet(&p, Duration::from_millis(800)) {
                    Ok(r) => println!(
                        "{id:04X}: OK  [{}]  {:?}",
                        protocol::hex(&r.payload),
                        String::from_utf8_lossy(&r.payload)
                    ),
                    Err(engine::ClientError::Rejected { code, .. }) => {
                        println!("{id:04X}: err {code}")
                    }
                    Err(e) => println!("{id:04X}: {e}"),
                }
            }
        }
        "discover" => {
            // read-only discovery: odd ("get") command ids in a feature, empty payload
            let feature: u16 = args
                .get(1)
                .and_then(|v| v.parse().ok())
                .ok_or("usage: discover <feature> [from_cmd] [to_cmd]")?;
            let from: u16 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(1);
            let to: u16 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(0x3F);
            let s = open(device.as_deref(), false)?;
            for cmd in (from..=to).filter(|c| c % 2 == 1) {
                let id = (feature << 9) | cmd;
                let p = Packet::new(protocol::ids::VENDOR_SENNHEISER, id, vec![]);
                match s.client.send_packet(&p, Duration::from_millis(800)) {
                    Ok(r) => println!("{id:04X}: OK  [{}]", protocol::hex(&r.payload)),
                    Err(engine::ClientError::Rejected { code, .. }) => {
                        println!("{id:04X}: err {code}")
                    }
                    Err(e) => println!("{id:04X}: {e}"),
                }
            }
        }
        "raw" => {
            let b = hex_bytes(args.get(1).ok_or("usage: raw <hex>")?).ok_or("bad hex")?;
            if b.len() < 4 {
                return Err("need at least vendor(2) + id(2)".into());
            }
            let p = Packet::new(
                u16::from_be_bytes([b[0], b[1]]),
                u16::from_be_bytes([b[2], b[3]]),
                b[4..].to_vec(),
            );
            let s = open(device.as_deref(), unsafe_ok)?;
            match s.client.send_packet(&p, Duration::from_millis(2000)) {
                Ok(r) => println!(
                    "<- {:04X}:{:04X} [{}] {:?}",
                    r.vendor,
                    r.id,
                    protocol::hex(&r.payload),
                    decode(&r, None)
                ),
                Err(e) => println!("ERROR {e}"),
            }
        }
        _ => {
            println!(
                "usage: hsctl [--device NAME] scan|info|monitor [secs]|get PROP|set PROP VALUE|eq|raw HEX [--unsafe]"
            );
            println!(
                "get props: {}",
                PROPS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
            );
            println!(
                "set props: adaptive, anti-wind, anc-mode M,S, anc-status, transparency, pause-on-full, sidetone, audio-mode, prompts, on-head, smart-pause, auto-call, comfort-call, touch-lock, bt-mode, auto-off, eq-submode, eq-band B,dB"
            );
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
