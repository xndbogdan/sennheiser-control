//! Tray right-click menu: built from the current state, ids are parsed back into actions.

use engine::{Action, AppState, LinkState, NoiseMode};
use protocol::{AntiWind, PromptMode, eq::PRESETS};
use tray_icon::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

use crate::settings::Settings;
use crate::shared::{
    COMFORT_CALLS_TITLE, PROMPTS_TITLE, SIDETONE_LABELS, SIDETONE_TITLE, Shared, UiCmd, WIND_ORDER,
    custom_level_label,
};

/// Custom-mode balance shortcuts (0 = full noise cancelling, 50 = neutral, 100 = full transparency).
const CUSTOM_LEVELS: [u8; 5] = [0, 25, 50, 75, 100];

fn esc(s: &str) -> String {
    s.replace('&', "&&")
}

fn check(id: &str, text: &str, checked: bool) -> CheckMenuItem {
    CheckMenuItem::with_id(id, esc(text), true, checked, None)
}

fn item(id: &str, text: &str, enabled: bool) -> MenuItem {
    MenuItem::with_id(id, esc(text), enabled, None)
}

fn submenu(text: &str, items: &[&dyn IsMenuItem]) -> Submenu {
    let s = Submenu::new(esc(text), true);
    for i in items {
        let _ = s.append(*i);
    }
    s
}

pub fn header_text(state: &AppState) -> String {
    let name = state
        .active_device()
        .map(|d| d.name.clone())
        .unwrap_or_else(|| "Headset".into());
    match (&state.link, state.live()) {
        (LinkState::Connected, Some(h)) => {
            let mut s = name;
            if let Some(p) = h.battery {
                s.push_str(&format!(" — {p}%"));
            }
            if h.charging_now() {
                s.push_str(" · Charging");
            }
            s
        }
        (LinkState::Connecting, _) => format!("{name} — connecting…"),
        (LinkState::Retrying(_), _) => format!("{name} — reconnecting…"),
        _ if state.connected_devices().next().is_none() => "No headset connected".into(),
        _ => name,
    }
}

pub fn tooltip(state: &AppState) -> String {
    let mut t = header_text(state);
    if let Some(h) = state.live()
        && let Some(m) = h.noise_mode()
    {
        t.push_str(" · ");
        t.push_str(m.label());
        if m == NoiseMode::Custom
            && let Some(level) = h.transparency
        {
            t.push_str(&format!(" ({})", custom_level_label(level).to_lowercase()));
        }
    }
    t
}

pub fn build(state: &AppState, settings: &Settings, autostart: bool) -> Menu {
    let menu = Menu::new();
    let header = item("header", &header_text(state), false);
    let _ = menu.append(&header);
    let _ = menu.append(&PredefinedMenuItem::separator());

    if let Some(h) = state.live() {
        let mode = h.noise_mode();
        let mut modes = vec![NoiseMode::Adaptive, NoiseMode::Custom];
        if h.caps.anc_off {
            modes.push(NoiseMode::Off);
        }
        let items: Vec<CheckMenuItem> = modes
            .iter()
            .map(|m| {
                check(
                    &format!("noise:{}", m.label().to_lowercase()),
                    m.label(),
                    mode == Some(*m),
                )
            })
            .collect();
        for i in &items {
            let _ = menu.append(i);
        }
        let custom = mode == Some(NoiseMode::Custom);
        let levels: Vec<CheckMenuItem> = CUSTOM_LEVELS
            .iter()
            .map(|l| {
                check(
                    &format!("level:{l}"),
                    &custom_level_label(*l),
                    custom && h.transparency == Some(*l),
                )
            })
            .collect();
        let refs: Vec<&dyn IsMenuItem> = levels.iter().map(|i| i as &dyn IsMenuItem).collect();
        let _ = menu.append(&submenu("Custom level", &refs));
        let winds: Vec<CheckMenuItem> = WIND_ORDER
            .iter()
            .map(|w| {
                check(
                    &format!("wind:{}", w.to_u8()),
                    w.label(),
                    h.anti_wind == Some(*w),
                )
            })
            .collect();
        let refs: Vec<&dyn IsMenuItem> = winds.iter().map(|i| i as &dyn IsMenuItem).collect();
        let _ = menu.append(&submenu("Wind noise reduction", &refs));
        let _ = menu.append(&PredefinedMenuItem::separator());

        // sound
        if h.caps.eq != protocol::EqGen::Unsupported && h.eq.known {
            use crate::ui::eq::EqChoice;
            let choice = crate::ui::eq::eq_choice(h, &h.eq.bands, &settings.custom_presets, false);
            let is = |c: EqChoice| choice.as_ref() == Some(&c);
            let mut presets = vec![check("eq:neutral", "Neutral", is(EqChoice::Neutral))];
            presets.extend(PRESETS.iter().enumerate().map(|(i, (name, _))| {
                check(
                    &format!("preset:{i}"),
                    name,
                    is(EqChoice::Preset(name.to_string())),
                )
            }));
            presets.extend(settings.custom_presets.iter().enumerate().map(|(i, p)| {
                check(
                    &format!("upreset:{i}"),
                    &p.name,
                    is(EqChoice::Preset(p.name.clone())),
                )
            }));
            presets.push(check("eq:custom", "Custom", is(EqChoice::Custom)));
            let refs: Vec<&dyn IsMenuItem> = presets.iter().map(|i| i as &dyn IsMenuItem).collect();
            let title = match &choice {
                Some(c) => format!("EQ preset ({})", c.label()),
                None => "EQ preset".to_string(),
            };
            let _ = menu.append(&submenu(&title, &refs));
            let _ = menu.append(&check(
                "bass",
                "Bass boost",
                h.eq.bass_boost && h.eq_active(),
            ));
        }
        let _ = menu.append(&check("podcast", "Podcast mode", h.podcast()));
        let _ = menu.append(&PredefinedMenuItem::separator());

        // settings submenu
        let s = Submenu::new("Headset settings", true);
        let c = &h.caps;
        if c.on_head_detection {
            let _ = s.append(&check(
                "onhead",
                "On-head detection",
                h.on_head_detection == Some(true),
            ));
            let on_head = h.on_head_detection == Some(true);
            let _ = s.append(&CheckMenuItem::with_id(
                "smartpause",
                "Smart pause",
                on_head,
                h.smart_pause == Some(true),
                None,
            ));
            if c.auto_answer_calls {
                let _ = s.append(&CheckMenuItem::with_id(
                    "autocall",
                    "Auto-answer calls",
                    on_head,
                    h.auto_call == Some(true),
                    None,
                ));
            }
        }
        let _ = s.append(&check(
            "comfort",
            COMFORT_CALLS_TITLE,
            h.comfort_call == Some(true),
        ));
        if c.touch_lock {
            let _ = s.append(&check(
                "touchlock",
                "Lock touch controls",
                h.touch_lock == Some(true),
            ));
        }
        let _ = s.append(&PredefinedMenuItem::separator());
        if c.auto_power_off {
            let offs: Vec<CheckMenuItem> = protocol::AUTO_POWER_OFF_CHOICES
                .iter()
                .map(|secs| {
                    let label = if *secs == 0 {
                        "Never".to_string()
                    } else {
                        format!("{} minutes", secs / 60)
                    };
                    check(
                        &format!("autooff:{secs}"),
                        &label,
                        h.auto_off_seconds == Some(*secs),
                    )
                })
                .collect();
            let refs: Vec<&dyn IsMenuItem> = offs.iter().map(|i| i as &dyn IsMenuItem).collect();
            let _ = s.append(&submenu("Auto power-off", &refs));
        }
        if c.voice_prompts {
            let prompts: Vec<CheckMenuItem> = PromptMode::ALL
                .iter()
                .map(|m| {
                    check(
                        &format!("prompts:{}", m.to_u8()),
                        m.label(),
                        h.prompt_mode == Some(*m),
                    )
                })
                .collect();
            let refs: Vec<&dyn IsMenuItem> = prompts.iter().map(|i| i as &dyn IsMenuItem).collect();
            let _ = s.append(&submenu(PROMPTS_TITLE, &refs));
        }
        let side: Vec<CheckMenuItem> = SIDETONE_LABELS
            .iter()
            .enumerate()
            .map(|(i, l)| check(&format!("sidetone:{i}"), l, h.sidetone == Some(i as u8)))
            .collect();
        let refs: Vec<&dyn IsMenuItem> = side.iter().map(|i| i as &dyn IsMenuItem).collect();
        let _ = s.append(&submenu(SIDETONE_TITLE, &refs));
        let _ = menu.append(&s);

        // multipoint: the hosts paired with the headset; click connects or disconnects
        if !h.paired_hosts.is_empty() {
            let full = h.paired_max_connections.is_some_and(|max| {
                h.paired_hosts.iter().filter(|x| x.connected).count() >= usize::from(max)
            });
            let hosts: Vec<CheckMenuItem> = h
                .paired_hosts
                .iter()
                .map(|host| {
                    let own = h.paired_own_index == Some(host.index);
                    let label = if own {
                        format!("{} (this PC)", host.name)
                    } else {
                        host.name.clone()
                    };
                    CheckMenuItem::with_id(
                        format!("host:{}", host.index),
                        esc(&label),
                        !own && (host.connected || !full),
                        host.connected,
                        None,
                    )
                })
                .collect();
            let refs: Vec<&dyn IsMenuItem> = hosts.iter().map(|i| i as &dyn IsMenuItem).collect();
            let _ = menu.append(&submenu("Connections", &refs));
        }
    }

    let connected: Vec<_> = state.connected_devices().collect();
    if connected.len() > 1 {
        let items: Vec<CheckMenuItem> = connected
            .iter()
            .map(|d| {
                check(
                    &format!("device:{}", d.id),
                    &state.display_name(d),
                    state.active.as_deref() == Some(d.id.as_str()),
                )
            })
            .collect();
        let refs: Vec<&dyn IsMenuItem> = items.iter().map(|i| i as &dyn IsMenuItem).collect();
        let _ = menu.append(&submenu("Headset", &refs));
    }
    if state.live().is_some() || connected.len() > 1 {
        let _ = menu.append(&PredefinedMenuItem::separator());
    }
    if state.connected_devices().next().is_none() {
        let _ = menu.append(&item("btsettings", "Bluetooth settings…", true));
    }
    let _ = menu.append(&item("open", "Open Sennheiser Control", true));
    let _ = menu.append(&item("settings", "Settings…", true));
    let _ = menu.append(&check("autostart", "Start with Windows", autostart));
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&item("quit", "Quit", true));
    menu
}

/// Execute a menu click.
pub fn handle(id: &str, shared: &Shared) {
    let state = shared.engine.snapshot();
    let h = state.live();
    let (key, arg) = id.split_once(':').unwrap_or((id, ""));
    let num = || arg.parse::<u32>().ok();
    let act = |a: Action| shared.dispatch(a);
    match key {
        "open" => shared.send_ui(UiCmd::Show),
        "settings" => shared.send_ui(UiCmd::OpenSettings),
        "quit" => shared.send_ui(UiCmd::Quit),
        "btsettings" => crate::platform::open_bluetooth_settings(),
        "autostart" => {
            let _ = crate::platform::set_autostart(!crate::platform::autostart_enabled());
            shared.wake_all();
        }
        "noise" => act(Action::SetNoiseMode(match arg {
            "custom" => NoiseMode::Custom,
            "off" => NoiseMode::Off,
            _ => NoiseMode::Adaptive,
        })),
        "level" => {
            if let Some(l) = num() {
                act(Action::SetTransparency(l.min(100) as u8));
            }
        }
        "wind" => {
            if let Some(w) = num().and_then(|n| AntiWind::from_u8(n as u8)) {
                act(Action::SetAntiWind(w));
            }
        }
        "preset" => {
            if let Some((_, bands)) = num().and_then(|i| PRESETS.get(i as usize)) {
                act(Action::SetEqBands(*bands));
            }
        }
        "upreset" => {
            if let Some(p) =
                num().and_then(|i| shared.settings().custom_presets.get(i as usize).cloned())
            {
                act(Action::SetEqBands(p.bands));
            }
        }
        "eq" => match arg {
            "neutral" => act(Action::SetEqEnabled(false)),
            "custom" => act(Action::SetEqBands(
                shared
                    .settings()
                    .custom_eq
                    .unwrap_or([0.0; protocol::eq::BANDS]),
            )),
            _ => {}
        },
        "bass" => act(Action::SetBassBoost(
            !h.is_some_and(|h| h.eq.bass_boost && h.eq_active()),
        )),
        "podcast" => act(Action::SetPodcast(!h.is_some_and(|h| h.podcast()))),
        "onhead" => act(Action::SetOnHeadDetection(
            h.and_then(|h| h.on_head_detection) != Some(true),
        )),
        "smartpause" => act(Action::SetSmartPause(
            h.and_then(|h| h.smart_pause) != Some(true),
        )),
        "autocall" => act(Action::SetAutoCall(
            h.and_then(|h| h.auto_call) != Some(true),
        )),
        "comfort" => act(Action::SetComfortCall(
            h.and_then(|h| h.comfort_call) != Some(true),
        )),
        "touchlock" => act(Action::SetTouchLock(
            h.and_then(|h| h.touch_lock) != Some(true),
        )),
        "autooff" => {
            if let Some(s) = num() {
                act(Action::SetAutoOff(s as u16));
            }
        }
        "prompts" => {
            if let Some(m) = num().and_then(|n| PromptMode::from_u8(n as u8)) {
                act(Action::SetPromptMode(m));
            }
        }
        "sidetone" => {
            if let Some(v) = num() {
                act(Action::SetSidetone(v as u8));
            }
        }
        "host" => {
            if let Some(i) = num().map(|i| i as u8)
                && let Some(host) = h.and_then(|h| h.paired_hosts.iter().find(|x| x.index == i))
            {
                act(if host.connected {
                    Action::DisconnectHost(i)
                } else {
                    Action::ConnectHost(i)
                });
            }
        }
        "device" => act(Action::SelectDevice(arg.to_string())),
        _ => {}
    }
    // menu check marks toggle locally; make sure the menu is rebuilt from the real state
    shared.wake_all();
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{DeviceEntry, HeadsetState, PairedHost};
    use protocol::{Update, Version};
    use tray_icon::menu::MenuItemKind;

    /// Keys `handle` understands; every clickable menu id must use one of them.
    const HANDLED: &[&str] = &[
        "open",
        "settings",
        "quit",
        "btsettings",
        "autostart",
        "noise",
        "level",
        "wind",
        "eq",
        "preset",
        "upreset",
        "bass",
        "podcast",
        "onhead",
        "smartpause",
        "autocall",
        "comfort",
        "touchlock",
        "autooff",
        "prompts",
        "sidetone",
        "host",
        "device",
    ];

    /// (depth, id, text, enabled, checked) for every item, submenus included.
    fn walk(
        items: Vec<MenuItemKind>,
        depth: usize,
        out: &mut Vec<(usize, String, String, bool, bool)>,
    ) {
        for i in items {
            match i {
                MenuItemKind::MenuItem(m) => {
                    out.push((depth, m.id().0.clone(), m.text(), m.is_enabled(), false))
                }
                MenuItemKind::Check(c) => out.push((
                    depth,
                    c.id().0.clone(),
                    c.text(),
                    c.is_enabled(),
                    c.is_checked(),
                )),
                MenuItemKind::Submenu(s) => {
                    out.push((depth, String::new(), s.text(), s.is_enabled(), false));
                    walk(s.items(), depth + 1, out);
                }
                _ => {}
            }
        }
    }

    fn custom_m4() -> AppState {
        let mut h = HeadsetState::default();
        for u in [
            Update::FirmwareVersion(Version::new(3, 38, 3)),
            Update::Battery(Some(80)),
            Update::AncStatus(true),
            Update::Adaptive(false),
            Update::Transparency(25),
            Update::Eq2Sliders(vec![0; 5]),
            Update::PairedOwnIndex(1),
            Update::PairedMaxConnections(2),
        ] {
            h.apply(&u);
        }
        h.paired_hosts = vec![
            PairedHost {
                index: 0,
                name: "Phone".into(),
                connected: false,
            },
            PairedHost {
                index: 1,
                name: "PC".into(),
                connected: true,
            },
        ];
        AppState {
            devices: vec![DeviceEntry {
                id: "m4".into(),
                name: "MOMENTUM 4".into(),
                address: "00:1b:66:00:00:01".into(),
                connected: true,
            }],
            active: Some("m4".into()),
            link: LinkState::Connected,
            headset: Some(h),
            discovery_done: true,
            ..Default::default()
        }
    }

    #[test]
    fn menu_mirrors_state_and_every_id_is_handled() {
        let state = custom_m4();
        let mut items = Vec::new();
        walk(
            build(&state, &Settings::default(), false).items(),
            0,
            &mut items,
        );
        // `HC_PRINT_MENU=1 cargo test -p app menu -- --nocapture` shows the whole tree
        if std::env::var_os("HC_PRINT_MENU").is_some() {
            for (depth, id, text, enabled, checked) in &items {
                let mark = if *checked { "✓" } else { " " };
                let dim = if *enabled { "" } else { "  (disabled)" };
                eprintln!("{}{mark} {text}{dim}   [{id}]", "    ".repeat(*depth));
            }
        }
        let find = |text: &str| {
            items
                .iter()
                .find(|i| i.2 == text)
                .unwrap_or_else(|| panic!("no menu item {text:?}"))
        };
        assert!(!find("MOMENTUM 4 — 80%").3, "header is informational");
        // noise modes: Custom checked, level 25 = "Noise cancelling 50%"
        assert!(find("Custom").4 && !find("Adaptive").4 && !find("Off").4);
        assert!(find("Noise cancelling 50%").4);
        assert_eq!(
            find("Noise cancelling 50%").0,
            1,
            "levels live in the Custom level submenu"
        );
        // multipoint: this PC can't be toggled, the idle phone can be connected
        assert!(!find("PC (this PC)").3 && find("PC (this PC)").4);
        assert!(find("Phone").3 && !find("Phone").4);
        assert!(tooltip(&state).ends_with("Custom (noise cancelling 50%)"));
        for (_, id, text, _, _) in &items {
            if id.is_empty() {
                continue;
            }
            let key = id.split_once(':').map_or(id.as_str(), |(k, _)| k);
            assert!(
                key == "header" || HANDLED.contains(&key),
                "menu item {text:?} has unhandled id {id:?}"
            );
        }
    }
}
