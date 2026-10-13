//! Pages and cards.

use std::time::Instant;

use egui::{Align, Align2, CornerRadius, Layout, RichText, Sense, Ui, pos2, vec2};
use egui_phosphor::regular as ph;
use engine::{Action, HeadsetState, LinkState, NoiseMode};
use protocol::{EqGen, PromptMode, eq::PRESETS};

use super::eq::EqChoice;
use super::theme::{Palette, bold, icon, regular, semibold};
use super::widgets::{self as w, card, section_title, setting_row, stacked_row, toggle};
use super::{HOST_PENDING_TIMEOUT, HeadsetApp, HostPending, Pending};
use crate::settings::{CustomPreset, ThemePref};
use crate::shared::{
    COMFORT_CALLS_TITLE, PROMPTS_TITLE, SIDETONE_LABELS, SIDETONE_TITLE, WIND_ORDER,
    custom_level_label,
};

const AUTO_OFF_LABELS: [&str; 4] = ["Never", "15 min", "30 min", "60 min"];
const LOW_BATTERY_CHOICES: [u8; 5] = [0, 10, 15, 20, 30];

fn centered(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.vertical_centered(|ui| add(ui));
}

impl HeadsetApp {
    pub(super) fn main_page(&mut self, ui: &mut Ui) {
        let p = self.palette;
        let st = self.state.clone();
        if !st.discovery_done {
            ui.add_space(120.0);
            centered(ui, |ui| {
                w::spinner(ui, &p, 28.0);
                ui.label(
                    RichText::new("Looking for your headset…")
                        .font(regular(14.0))
                        .color(p.text_dim),
                );
            });
            return;
        }
        let Some(active) = st.active_device().cloned() else {
            self.empty_state(ui, "No MOMENTUM 4 paired", "Pair your headset in Windows Bluetooth settings. It will show up here automatically.");
            return;
        };
        if !active.connected {
            self.empty_state(
                ui,
                &format!("{} isn't connected", active.name),
                "Turn the headset on, or connect it from Bluetooth settings.",
            );
            return;
        }
        match (&st.link, st.live()) {
            (LinkState::Connected, Some(h)) => {
                let h = h.clone();
                self.header_card(ui, &active.name, Some(&h));
                self.noise_card(ui, &h);
                self.sound_card(ui, &h);
                self.calls_card(ui, &h);
                self.controls_card(ui, &h);
                self.connections_card(ui, &h);
                self.about_card(ui, &h);
            }
            (LinkState::Retrying(reason), _) => {
                self.header_card(ui, &active.name, None);
                let reason = reason.clone();
                card(ui, &p, |ui| {
                    section_title(ui, &p, ph::PLUG, "Can't reach the headset");
                    ui.label(RichText::new("Windows is connected to the headset, but its control channel didn't answer. Retrying automatically.").font(regular(13.0)).color(p.text_dim));
                    ui.label(
                        RichText::new(reason)
                            .font(regular(12.0))
                            .color(p.text_faint),
                    );
                    ui.add_space(4.0);
                    if w::primary_button(ui, &p, Some(ph::ARROWS_CLOCKWISE), "Retry now").clicked()
                    {
                        self.act(Action::Reconnect);
                    }
                });
            }
            _ => {
                self.header_card(ui, &active.name, None);
                card(ui, &p, |ui| {
                    ui.horizontal(|ui| {
                        w::spinner(ui, &p, 18.0);
                        ui.label(
                            RichText::new("Connecting to the headset…")
                                .font(regular(14.0))
                                .color(p.text_dim),
                        );
                    });
                });
            }
        }
    }

    fn empty_state(&mut self, ui: &mut Ui, title: &str, body: &str) {
        let p = self.palette;
        let st = self.state.clone();
        ui.add_space(40.0);
        centered(ui, |ui| {
            let (r, _) = ui.allocate_exact_size(vec2(132.0, 132.0), Sense::hover());
            w::headphones_art(ui.painter(), r, &p, true);
            ui.add_space(10.0);
            ui.label(RichText::new(title).font(semibold(18.0)).color(p.text));
            ui.add(
                egui::Label::new(RichText::new(body).font(regular(13.0)).color(p.text_dim)).wrap(),
            );
            ui.add_space(8.0);
            if w::primary_button(ui, &p, Some(ph::BLUETOOTH), "Open Bluetooth settings").clicked() {
                crate::platform::open_bluetooth_settings();
            }
        });
        if !st.devices.is_empty() {
            ui.add_space(12.0);
            card(ui, &p, |ui| {
                ui.label(
                    RichText::new("Paired headsets")
                        .font(semibold(13.0))
                        .color(p.text_dim),
                );
                for d in &st.devices {
                    ui.horizontal(|ui| {
                        w::status_dot(ui, if d.connected { p.good } else { p.text_faint });
                        ui.label(RichText::new(&d.name).font(regular(14.0)).color(p.text));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let s = if d.connected {
                                "Connected"
                            } else {
                                "Not connected"
                            };
                            ui.label(RichText::new(s).font(regular(12.0)).color(p.text_faint));
                        });
                    });
                }
            });
        }
    }

    fn header_card(&mut self, ui: &mut Ui, name: &str, h: Option<&HeadsetState>) {
        let p = self.palette;
        let st = self.state.clone();
        let multiple = st.connected_devices().count() > 1;
        card(ui, &p, |ui| {
            ui.horizontal(|ui| {
                let (art, _) = ui.allocate_exact_size(vec2(68.0, 68.0), Sense::hover());
                w::headphones_art(ui.painter(), art, &p, h.is_none());
                ui.add_space(6.0);
                let middle_w = (ui.available_width() - 74.0).max(80.0);
                ui.allocate_ui_with_layout(
                    vec2(middle_w, 68.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.spacing_mut().item_spacing.y = 3.0;
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            let title = ui.add(
                                egui::Label::new(
                                    RichText::new(name).font(semibold(19.0)).color(p.text),
                                )
                                .truncate()
                                .sense(if multiple {
                                    Sense::click()
                                } else {
                                    Sense::hover()
                                }),
                            );
                            if multiple {
                                let chev = ui.add(
                                    egui::Label::new(
                                        RichText::new(if self.show_devices {
                                            ph::CARET_UP
                                        } else {
                                            ph::CARET_DOWN
                                        })
                                        .font(icon(15.0))
                                        .color(p.text_dim),
                                    )
                                    .sense(Sense::click()),
                                );
                                if title.clicked() || chev.clicked() {
                                    self.show_devices = !self.show_devices;
                                }
                                title.on_hover_cursor(egui::CursorIcon::PointingHand);
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 5.0;
                            let (dot, status) = match h {
                                Some(h) if h.charging_now() => (p.good, "Charging".to_string()),
                                Some(_) => (p.good, "Connected".to_string()),
                                None => (p.warn, "Connecting".to_string()),
                            };
                            w::status_dot(ui, dot);
                            let variant = h
                                .and_then(|h| h.variant())
                                .map(|v| format!(" · {v}"))
                                .unwrap_or_default();
                            ui.label(
                                RichText::new(format!("{status}{variant}"))
                                    .font(regular(13.0))
                                    .color(p.text_dim),
                            );
                        });
                        if let Some(h) = h {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(5.0, 5.0);
                                if let Some(c) = h.codec {
                                    w::chip(ui, &p, Some(ph::WAVEFORM), c.label());
                                }
                                if let Some(sr) = h.sample_rate {
                                    let khz = sr as f32 / 1000.0;
                                    let text = if khz.fract() == 0.0 {
                                        format!("{khz:.0} kHz")
                                    } else {
                                        format!("{khz:.1} kHz")
                                    };
                                    w::chip(ui, &p, None, &text);
                                }
                            });
                        }
                    },
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    w::battery_ring(
                        ui,
                        &p,
                        h.and_then(|h| h.battery),
                        h.is_some_and(|h| h.charging_now()),
                        64.0,
                    )
                    .on_hover_text("Battery");
                });
            });
            if multiple && self.show_devices {
                w::divider(ui, &p);
                for d in st.connected_devices() {
                    let selected = st.active.as_deref() == Some(d.id.as_str());
                    let (r, resp) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
                    if resp.hovered() || selected {
                        ui.painter().rect_filled(
                            r,
                            CornerRadius::same(8),
                            if selected {
                                p.accent_soft()
                            } else {
                                p.surface2
                            },
                        );
                    }
                    ui.painter().text(
                        pos2(r.left() + 12.0, r.center().y),
                        Align2::LEFT_CENTER,
                        ph::HEADPHONES,
                        icon(15.0),
                        if selected { p.accent } else { p.text_dim },
                    );
                    ui.painter().text(
                        pos2(r.left() + 34.0, r.center().y),
                        Align2::LEFT_CENTER,
                        st.display_name(d),
                        regular(14.0),
                        p.text,
                    );
                    if selected {
                        ui.painter().text(
                            pos2(r.right() - 12.0, r.center().y),
                            Align2::RIGHT_CENTER,
                            ph::CHECK,
                            icon(15.0),
                            p.accent,
                        );
                    }
                    if resp
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.act(Action::SelectDevice(d.id.clone()));
                        self.show_devices = false;
                    }
                }
            }
        });
    }

    fn noise_card(&mut self, ui: &mut Ui, h: &HeadsetState) {
        let p = self.palette;
        card(ui, &p, |ui| {
            section_title(ui, &p, ph::EAR, "Noise control");
            // Smart Control Plus "NoiseControl" variant 2: adaptive, custom, and off from fw 2.13.32
            let mut modes = vec![NoiseMode::Adaptive, NoiseMode::Custom];
            if h.caps.anc_off {
                modes.push(NoiseMode::Off);
            }
            let labels: Vec<&str> = modes.iter().map(|m| m.label()).collect();
            let mode = h.noise_mode();
            let sel = mode.and_then(|m| modes.iter().position(|x| *x == m));
            if let Some(i) = w::segmented(ui, &p, "noise-mode", &labels, sel, mode.is_some(), 36.0)
            {
                self.transparency = None;
                self.act(Action::SetNoiseMode(modes[i]));
            }
            ui.add_space(2.0);
            match mode {
                Some(NoiseMode::Custom) => self.custom_balance(ui, h),
                Some(NoiseMode::Adaptive) => hint(
                    ui,
                    &p,
                    "Noise cancelling adjusts itself to your surroundings.",
                ),
                Some(NoiseMode::Off) => hint(
                    ui,
                    &p,
                    "Noise cancelling and transparency are off. Saves a little battery.",
                ),
                None => {}
            }
            w::divider(ui, &p);
            let wind_sel = h
                .anti_wind
                .and_then(|a| WIND_ORDER.iter().position(|x| *x == a));
            // part of noise cancelling: nothing to reduce while noise control is Off
            let anc_running = mode != Some(NoiseMode::Off);
            stacked_row(ui, &p, ph::WIND, "Wind noise reduction", None, |ui| {
                if let Some(i) = w::segmented(
                    ui,
                    &p,
                    "wind",
                    &WIND_ORDER.map(|w| w.label()),
                    wind_sel,
                    h.anti_wind.is_some() && anc_running,
                    30.0,
                ) {
                    self.act(Action::SetAntiWind(WIND_ORDER[i]));
                }
            });
            if !anc_running {
                hint(ui, &p, "Works with Adaptive and Custom.");
            }
        });
    }

    /// Custom mode: noise cancelling ⟷ transparency balance.
    /// Custom mode: one balance from noise cancelling through neutral (centre) to transparency, as in Smart
    /// Control Plus (`ANC_Transparency` = slider × 100).
    fn custom_balance(&mut self, ui: &mut Ui, h: &HeadsetState) {
        let p = self.palette;
        let device_t = h.transparency.unwrap_or(50) as f32;
        let mut t = self
            .transparency
            .as_ref()
            .map(|x| x.value)
            .unwrap_or(device_t);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(custom_level_label(t.round() as u8))
                    .font(semibold(13.0))
                    .color(p.text),
            );
        });
        let r = w::balance_slider(ui, &p, &mut t, h.transparency.is_some());
        let dragging = r.dragged();
        let done = r.drag_stopped() || (r.clicked() && !dragging);
        if r.changed() || done {
            self.transparency = Some(Pending {
                value: t,
                since: Instant::now(),
                dragging: dragging && !done,
            });
            if self.throttled(done) {
                self.act(Action::SetTransparency(t.round() as u8));
            }
        }
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Noise cancelling")
                    .font(regular(11.5))
                    .color(p.text_faint),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("Transparency")
                        .font(regular(11.5))
                        .color(p.text_faint),
                );
            });
        });
    }

    fn calls_card(&mut self, ui: &mut Ui, h: &HeadsetState) {
        let p = self.palette;
        let on_head = h.on_head_detection == Some(true);
        card(ui, &p, |ui| {
            section_title(ui, &p, ph::PHONE, "Calls");
            let mut cc = h.comfort_call == Some(true);
            setting_row(
                ui,
                &p,
                ph::USERS_THREE,
                COMFORT_CALLS_TITLE,
                Some("Hear your surroundings during calls"),
                h.comfort_call.is_some(),
                |ui| {
                    if toggle(ui, &p, &mut cc, h.comfort_call.is_some()).changed() {
                        self.act(Action::SetComfortCall(cc));
                    }
                },
            );
            if h.caps.on_head_detection && h.caps.auto_answer_calls {
                let mut ac = h.auto_call == Some(true);
                setting_row(
                    ui,
                    &p,
                    ph::PHONE_INCOMING,
                    "Auto-answer calls",
                    Some(if on_head {
                        "Answer by putting the headphones on"
                    } else {
                        "Needs on-head detection"
                    }),
                    on_head,
                    |ui| {
                        if toggle(ui, &p, &mut ac, on_head).changed() {
                            self.act(Action::SetAutoCall(ac));
                        }
                    },
                );
            }
            stacked_row(ui, &p, ph::MICROPHONE, SIDETONE_TITLE, None, |ui| {
                let sel = h
                    .sidetone
                    .map(|v| v.min(engine::state::SIDETONE_MAX) as usize);
                if let Some(i) = w::segmented(
                    ui,
                    &p,
                    "sidetone",
                    &SIDETONE_LABELS,
                    sel,
                    h.sidetone.is_some(),
                    30.0,
                ) {
                    self.act(Action::SetSidetone(i as u8));
                }
            });
        });
    }

    fn sound_card(&mut self, ui: &mut Ui, h: &HeadsetState) {
        let p = self.palette;
        card(ui, &p, |ui| {
            section_title(ui, &p, ph::EQUALIZER, "Sound");
            let podcast = h.podcast();
            let mut pod = podcast;
            setting_row(
                ui,
                &p,
                ph::MICROPHONE_STAGE,
                "Podcast mode",
                Some("Clearer voices for podcasts and audiobooks"),
                h.audio_mode.is_some(),
                |ui| {
                    if toggle(ui, &p, &mut pod, h.audio_mode.is_some()).changed() {
                        self.act(Action::SetPodcast(pod));
                    }
                },
            );
            if h.caps.eq == EqGen::Unsupported {
                ui.label(
                    RichText::new("The equalizer needs headset firmware 2.12 or newer.")
                        .font(regular(13.0))
                        .color(p.text_dim),
                );
                return;
            }
            w::divider(ui, &p);
            let enabled = !podcast && h.eq.known;
            let editing = self.eq_bands.is_some();
            // with the EQ off the headset plays its own tuning, so the graph is flat until a curve is chosen
            let mut bands = match &self.eq_bands {
                Some(e) => e.value,
                None if h.eq_active() => h.eq.bands,
                None => [0.0; protocol::eq::BANDS],
            };
            let choice = super::eq::eq_choice(h, &bands, &self.settings.custom_presets, editing);
            // a curve that matches no preset (from here or from the phone) becomes the Custom curve
            if !editing
                && h.eq_active()
                && choice == Some(EqChoice::Custom)
                && self.settings.custom_eq != Some(bands)
            {
                self.shared.update_settings(|s| s.custom_eq = Some(bands));
            }
            let custom_curve = self
                .settings
                .custom_eq
                .unwrap_or([0.0; protocol::eq::BANDS]);
            let selected = |c: &EqChoice| choice.as_ref() == Some(c);

            // preset chips: Neutral, Sennheiser presets, saved presets, Custom
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                let mut apply = None;
                let mut delete = None;
                if preset_chip(ui, &p, "Neutral", selected(&EqChoice::Neutral), enabled)
                    .on_hover_text("The headset's own tuning, equalizer off")
                    .clicked()
                {
                    self.eq_bands = None;
                    self.act(Action::SetEqEnabled(false));
                }
                for (name, preset) in PRESETS {
                    let c = EqChoice::Preset(name.to_string());
                    if preset_chip(ui, &p, name, selected(&c), enabled).clicked() {
                        apply = Some(*preset);
                    }
                }
                for (i, cp) in self.settings.custom_presets.iter().enumerate() {
                    let c = EqChoice::Preset(cp.name.clone());
                    let r = preset_chip(ui, &p, &cp.name, selected(&c), enabled);
                    if r.clicked() {
                        apply = Some(cp.bands);
                    }
                    r.context_menu(|ui| {
                        if ui.button("Delete preset").clicked() {
                            delete = Some(i);
                            ui.close();
                        }
                    });
                }
                if preset_chip(ui, &p, "Custom", selected(&EqChoice::Custom), enabled)
                    .on_hover_text("Your own curve: drag the points to change it")
                    .clicked()
                {
                    apply = Some(custom_curve);
                }
                if let Some(b) = apply {
                    bands = b;
                    self.eq_bands = Some(Pending {
                        value: b,
                        since: Instant::now(),
                        dragging: false,
                    });
                    self.act(Action::SetEqBands(b));
                }
                if let Some(i) = delete {
                    self.shared.update_settings(|s| {
                        s.custom_presets.remove(i);
                    });
                }
            });
            ui.add_space(4.0);

            let edit = super::eq::editor(ui, &p, &mut bands, h.eq.min_db, h.eq.max_db, enabled);
            if enabled {
                ui.label(
                    RichText::new("Drag points · Shift for fine steps")
                        .font(regular(11.5))
                        .color(p.text_faint),
                );
            }
            if edit.changed || edit.released {
                self.eq_bands = Some(Pending {
                    value: bands,
                    since: Instant::now(),
                    dragging: !edit.released,
                });
                if self.throttled(edit.released) {
                    self.act(Action::SetEqBands(bands));
                }
                if edit.released
                    && super::eq::matching_preset(&bands, &self.settings.custom_presets).is_none()
                {
                    self.shared.update_settings(|s| s.custom_eq = Some(bands));
                }
            }
            if podcast {
                hint(
                    ui,
                    &p,
                    "Podcast mode uses a voice-optimised sound; the equalizer is paused.",
                );
            }

            // bass boost is part of the headset's user EQ, so it can't apply on Neutral (EQ off)
            let neutral = choice == Some(EqChoice::Neutral);
            let bass_enabled = enabled && !neutral;
            let mut bass = h.eq.bass_boost && h.eq_active();
            setting_row(
                ui,
                &p,
                ph::SPEAKER_SIMPLE_HIGH,
                "Bass boost",
                if podcast {
                    Some("Paused in podcast mode")
                } else if neutral {
                    Some("Pick a preset or Custom to use it")
                } else {
                    None
                },
                bass_enabled,
                |ui| {
                    if toggle(ui, &p, &mut bass, bass_enabled).changed() {
                        self.act(Action::SetBassBoost(bass));
                    }
                },
            );

            let custom_tools = choice == Some(EqChoice::Custom) && enabled;
            if custom_tools || self.new_preset_name.is_some() {
                ui.horizontal(|ui| {
                    if let Some(name) = &mut self.new_preset_name {
                        let resp = ui.add(
                            egui::TextEdit::singleline(name)
                                .hint_text("Preset name")
                                .desired_width(ui.available_width() - 150.0)
                                .font(regular(13.0)),
                        );
                        resp.request_focus();
                        let save = w::ghost_button(ui, &p, Some(ph::CHECK), "Save").clicked()
                            || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                        let cancel = w::ghost_button(ui, &p, None, "Cancel").clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Escape));
                        if save && !name.trim().is_empty() {
                            let preset = CustomPreset {
                                name: name.trim().chars().take(24).collect(),
                                bands,
                            };
                            self.shared.update_settings(|s| {
                                s.custom_presets.retain(|c| c.name != preset.name);
                                s.custom_presets.push(preset);
                            });
                            self.new_preset_name = None;
                        } else if cancel {
                            self.new_preset_name = None;
                        }
                    } else {
                        if custom_tools
                            && w::ghost_button(ui, &p, Some(ph::ARROW_COUNTER_CLOCKWISE), "Flatten")
                                .on_hover_text("Set every band of the Custom curve to 0 dB")
                                .clicked()
                        {
                            let flat = [0.0; protocol::eq::BANDS];
                            self.eq_bands = Some(Pending {
                                value: flat,
                                since: Instant::now(),
                                dragging: false,
                            });
                            self.act(Action::SetEqBands(flat));
                            self.shared.update_settings(|s| s.custom_eq = Some(flat));
                        }
                        if custom_tools
                            && w::ghost_button(ui, &p, Some(ph::FLOPPY_DISK), "Save as preset")
                                .clicked()
                        {
                            self.new_preset_name = Some(String::new());
                        }
                    }
                });
            }
            if h.caps.sound_personalization || h.caps.hi_res_audio {
                w::divider(ui, &p);
            }
            if h.caps.sound_personalization && h.personalized_sound.is_some() {
                self.personalization_row(ui, h);
            }
            if h.caps.hi_res_audio && h.bt_mode.is_some() {
                self.hi_res_row(ui, h);
            }
        });
    }

    fn personalization_row(&mut self, ui: &mut Ui, h: &HeadsetState) {
        use protocol::PersonalizedSoundMode as Ps;
        let p = self.palette;
        let hi_res = h.hi_res() == Some(true);
        let ready = h.personalization_ready();
        let mut on = matches!(h.personalized_sound, Some(Ps::Enabled | Ps::Standby));
        let enabled = ready && !hi_res;
        let subtitle = match h.personalized_sound {
            Some(Ps::NotParameterized) => "Create a profile in the Smart Control app on your phone",
            Some(Ps::Calibrating) => "Calibrating…",
            _ if hi_res => "Not available with Hi-Res",
            _ => "Your hearing profile from Smart Control",
        };
        setting_row(
            ui,
            &p,
            ph::EAR,
            "Sound personalization",
            Some(subtitle),
            enabled,
            |ui| {
                if toggle(ui, &p, &mut on, enabled).changed() {
                    self.act(Action::SetPersonalizedSound(on));
                }
            },
        );
    }

    fn hi_res_row(&mut self, ui: &mut Ui, h: &HeadsetState) {
        let p = self.palette;
        let mut on = h.hi_res() == Some(true);
        setting_row(
            ui,
            &p,
            ph::WAVEFORM,
            "Hi-Res 24 bit/96 kHz",
            Some("High resolution audio if supported by the source"),
            true,
            |ui| {
                if toggle(ui, &p, &mut on, !self.confirm_hi_res).changed() {
                    if on {
                        // Smart Control Plus asks for confirmation before enabling
                        self.confirm_hi_res = true;
                    } else {
                        self.act(Action::SetHiRes(false));
                    }
                }
            },
        );
        if self.confirm_hi_res {
            confirm_box(
                ui,
                &p,
                ph::WARNING,
                "Enabling Hi-Res",
                "Hi-Res 24 bit/96 kHz will allow you to use high quality sound codecs, but might give a less stable Bluetooth connection, which increases the likelihood of dropouts.",
                |ui| {
                    if w::primary_button(ui, &p, None, "I understand").clicked() {
                        self.confirm_hi_res = false;
                        self.act(Action::SetHiRes(true));
                    }
                    if w::ghost_button(ui, &p, None, "Cancel").clicked() {
                        self.confirm_hi_res = false;
                    }
                },
            );
        }
    }

    fn controls_card(&mut self, ui: &mut Ui, h: &HeadsetState) {
        let p = self.palette;
        let c = h.caps;
        card(ui, &p, |ui| {
            section_title(ui, &p, ph::SLIDERS_HORIZONTAL, "Headset");
            let on_head = h.on_head_detection == Some(true);
            if c.on_head_detection {
                let mut v = on_head;
                setting_row(
                    ui,
                    &p,
                    ph::USER_FOCUS,
                    "On-head detection",
                    Some("Plays, pauses and powers on as you wear them"),
                    true,
                    |ui| {
                        if toggle(ui, &p, &mut v, h.on_head_detection.is_some()).changed() {
                            self.act(Action::SetOnHeadDetection(v));
                        }
                    },
                );
                let mut sp = h.smart_pause == Some(true);
                setting_row(
                    ui,
                    &p,
                    ph::PAUSE,
                    "Smart pause",
                    Some(if on_head {
                        "Pause when you take them off"
                    } else {
                        "Needs on-head detection"
                    }),
                    on_head,
                    |ui| {
                        if toggle(ui, &p, &mut sp, on_head).changed() {
                            self.act(Action::SetSmartPause(sp));
                        }
                    },
                );
            }
            if c.touch_lock {
                let mut tl = h.touch_lock == Some(true);
                setting_row(
                    ui,
                    &p,
                    ph::LOCK_SIMPLE,
                    "Lock touch controls",
                    Some(if c.touch_lock_persists {
                        "Ignore touches; stays locked after a restart"
                    } else {
                        "Ignore taps and swipes on the earcup"
                    }),
                    h.touch_lock.is_some(),
                    |ui| {
                        if toggle(ui, &p, &mut tl, h.touch_lock.is_some()).changed() {
                            self.act(Action::SetTouchLock(tl));
                        }
                    },
                );
            }
            if expander(ui, &p, ph::HAND_TAP, "Touch gestures", self.show_gestures) {
                self.show_gestures = !self.show_gestures;
            }
            if self.show_gestures {
                gesture_reference(ui, &p);
            }
            w::divider(ui, &p);
            if c.auto_power_off {
                let sel = h.auto_off_seconds.and_then(|s| {
                    protocol::AUTO_POWER_OFF_CHOICES
                        .iter()
                        .position(|x| *x == s)
                });
                stacked_row(ui, &p, ph::POWER, "Auto power-off", None, |ui| {
                    if let Some(i) = w::segmented(
                        ui,
                        &p,
                        "auto-off",
                        &AUTO_OFF_LABELS,
                        sel,
                        h.auto_off_seconds.is_some(),
                        30.0,
                    ) {
                        self.act(Action::SetAutoOff(protocol::AUTO_POWER_OFF_CHOICES[i]));
                    }
                });
            }
            if c.voice_prompts {
                let sel = h.prompt_mode.map(|m| m.to_u8() as usize);
                stacked_row(ui, &p, ph::CHAT_CIRCLE_DOTS, PROMPTS_TITLE, None, |ui| {
                    if let Some(i) = w::segmented(
                        ui,
                        &p,
                        "prompts",
                        &PromptMode::ALL
                            .iter()
                            .map(|m| m.label())
                            .collect::<Vec<_>>(),
                        sel,
                        h.prompt_mode.is_some(),
                        30.0,
                    ) && let Some(m) = PromptMode::from_u8(i as u8)
                    {
                        self.act(Action::SetPromptMode(m));
                    }
                });
            }
        });
    }

    /// Multipoint: devices paired with the headset, connect or disconnect them.
    fn connections_card(&mut self, ui: &mut Ui, h: &HeadsetState) {
        if h.paired_hosts.is_empty() {
            return;
        }
        let p = self.palette;
        let connected = h.paired_hosts.iter().filter(|x| x.connected).count();
        let full = h
            .paired_max_connections
            .is_some_and(|max| connected >= usize::from(max));
        card(ui, &p, |ui| {
            section_title(ui, &p, ph::DEVICES, "Connections");
            if let Some(max) = h.paired_max_connections {
                hint(
                    ui,
                    &p,
                    &format!("Up to {max} devices can be connected at the same time."),
                );
            }
            // a connect/disconnect is in flight until the headset reports the new state (or gives up)
            if let Some(hp) = self.host_pending
                && (hp.since.elapsed() > HOST_PENDING_TIMEOUT
                    || h.paired_hosts
                        .iter()
                        .any(|x| x.index == hp.index && x.connected == hp.connect))
            {
                self.host_pending = None;
            }
            if self.host_pending.is_some() {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(250));
            }
            for host in &h.paired_hosts {
                let own = h.paired_own_index == Some(host.index);
                let pending = self.host_pending.filter(|hp| hp.index == host.index);
                let subtitle = match (own, pending, host.connected) {
                    (true, _, _) => "This PC",
                    (false, Some(hp), _) if hp.connect => "Connecting…",
                    (false, Some(_), _) => "Disconnecting…",
                    (false, None, true) => "Connected",
                    (false, None, false) => "Not connected",
                };
                setting_row(
                    ui,
                    &p,
                    if own { ph::DESKTOP } else { ph::DEVICE_MOBILE },
                    &host.name,
                    Some(subtitle),
                    true,
                    |ui| {
                        if own {
                            w::status_dot(ui, p.good);
                        } else if pending.is_some() {
                            w::spinner(ui, &p, 18.0);
                        } else if host.connected {
                            if w::ghost_button(ui, &p, None, "Disconnect").clicked() {
                                self.host_pending = Some(HostPending {
                                    index: host.index,
                                    connect: false,
                                    since: Instant::now(),
                                });
                                self.act(Action::DisconnectHost(host.index));
                            }
                        } else {
                            // right to left: forget, then connect
                            if forget_button(ui, &p).clicked() {
                                self.confirm_forget = Some(host.index);
                            }
                            if full {
                                ui.label(
                                    RichText::new("Slots full")
                                        .font(regular(12.0))
                                        .color(p.text_faint),
                                )
                                .on_hover_text("Disconnect another device first");
                            } else if w::ghost_button(ui, &p, None, "Connect").clicked() {
                                self.host_pending = Some(HostPending {
                                    index: host.index,
                                    connect: true,
                                    since: Instant::now(),
                                });
                                self.act(Action::ConnectHost(host.index));
                            }
                        }
                    },
                );
                if self.confirm_forget == Some(host.index) && !host.connected {
                    confirm_box(
                        ui,
                        &p,
                        ph::TRASH,
                        &format!("Forget {}?", host.name),
                        "The headset removes this device from its pairing list. To use them together again, pair \
                         the headset from that device.",
                        |ui| {
                            if danger_button(ui, &p, "Forget device").clicked() {
                                self.confirm_forget = None;
                                self.act(Action::ForgetHost(host.index));
                            }
                            if w::ghost_button(ui, &p, None, "Cancel").clicked() {
                                self.confirm_forget = None;
                            }
                        },
                    );
                }
            }
        });
    }

    fn about_card(&mut self, ui: &mut Ui, h: &HeadsetState) {
        let p = self.palette;
        card(ui, &p, |ui| {
            let (r, resp) =
                ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
            ui.painter().text(
                pos2(r.left(), r.center().y),
                Align2::LEFT_CENTER,
                ph::INFO,
                icon(18.0),
                p.accent,
            );
            ui.painter().text(
                pos2(r.left() + 26.0, r.center().y),
                Align2::LEFT_CENTER,
                "About this headset",
                semibold(15.0),
                p.text,
            );
            ui.painter().text(
                pos2(r.right(), r.center().y),
                Align2::RIGHT_CENTER,
                if self.show_about {
                    ph::CARET_UP
                } else {
                    ph::CARET_DOWN
                },
                icon(15.0),
                p.text_dim,
            );
            if resp
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                self.show_about = !self.show_about;
            }
            if !self.show_about {
                return;
            }
            ui.add_space(4.0);
            let row = |ui: &mut Ui, k: &str, v: String| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(k).font(regular(13.0)).color(p.text_dim));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(v).font(regular(13.0)).color(p.text))
                                .selectable(true),
                        );
                    });
                });
            };
            let dash = || "—".to_string();
            row(ui, "Model", h.model_id.clone().unwrap_or_else(dash));
            row(
                ui,
                "Firmware",
                h.firmware.map(|v| v.to_string()).unwrap_or_else(dash),
            );
            row(ui, "Serial number", h.serial.clone().unwrap_or_else(dash));
            row(
                ui,
                "Hardware revision",
                h.hw_revision.map(|v| v.to_string()).unwrap_or_else(dash),
            );
            row(
                ui,
                "Codec",
                h.codec.map(|c| c.label().to_string()).unwrap_or_else(dash),
            );
            row(
                ui,
                "Sample rate",
                h.sample_rate
                    .map(|r| format!("{} Hz", r))
                    .unwrap_or_else(dash),
            );
            row(
                ui,
                "Equalizer",
                match h.caps.eq {
                    EqGen::Gen2 => "5-band (gen 2)".into(),
                    EqGen::Gen1 => "5-band (gen 1)".into(),
                    EqGen::Unsupported => "Not supported".into(),
                },
            );
            w::divider(ui, &p);
            if self.confirm_reset {
                confirm_box(
                    ui,
                    &p,
                    ph::WARNING,
                    "Factory reset the headset?",
                    "All headset settings return to their defaults and every paired device is removed, including this PC. Afterwards, pair the headset again in Windows Bluetooth settings.",
                    |ui| {
                        if danger_button(ui, &p, "Reset headset").clicked() {
                            self.confirm_reset = false;
                            self.act(Action::FactoryReset);
                        }
                        if w::ghost_button(ui, &p, None, "Cancel").clicked() {
                            self.confirm_reset = false;
                        }
                    },
                );
            } else if w::ghost_button(ui, &p, Some(ph::ARROW_COUNTER_CLOCKWISE), "Factory reset…")
                .clicked()
            {
                self.confirm_reset = true;
            }
        });
    }

    pub(super) fn settings_page(&mut self, ui: &mut Ui) {
        let p = self.palette;
        let s = self.settings.clone();
        ui.label(RichText::new("Settings").font(bold(22.0)).color(p.text));
        card(ui, &p, |ui| {
            section_title(ui, &p, ph::APP_WINDOW, "App");
            let mut auto = self.autostart;
            setting_row(
                ui,
                &p,
                ph::ROCKET_LAUNCH,
                "Start with Windows",
                Some("Starts quietly in the tray"),
                true,
                |ui| {
                    if toggle(ui, &p, &mut auto, true).changed() {
                        if let Err(e) = crate::platform::set_autostart(auto) {
                            log::warn!("autostart: {e}");
                        }
                        self.autostart = crate::platform::autostart_enabled();
                        self.shared.wake_all();
                    }
                },
            );
            stacked_row(ui, &p, ph::X_CIRCLE, "Close button", None, |ui| {
                if let Some(i) = w::segmented(
                    ui,
                    &p,
                    "close",
                    &["Hide to tray", "Quit"],
                    Some(usize::from(!s.close_to_tray)),
                    true,
                    30.0,
                ) {
                    self.shared.update_settings(|s| s.close_to_tray = i == 0);
                }
            });
            stacked_row(ui, &p, ph::PALETTE, "Theme", None, |ui| {
                let sel = match s.theme {
                    ThemePref::System => 0,
                    ThemePref::Dark => 1,
                    ThemePref::Light => 2,
                };
                if let Some(i) = w::segmented(
                    ui,
                    &p,
                    "theme",
                    &["System", "Dark", "Light"],
                    Some(sel),
                    true,
                    30.0,
                ) {
                    let t = [ThemePref::System, ThemePref::Dark, ThemePref::Light][i];
                    self.shared.update_settings(|s| s.theme = t);
                }
            });
        });
        card(ui, &p, |ui| {
            section_title(ui, &p, ph::TRAY, "Tray");
            stacked_row(
                ui,
                &p,
                ph::BATTERY_WARNING,
                "Low battery alert",
                None,
                |ui| {
                    let labels = ["Off", "10%", "15%", "20%", "30%"];
                    let sel = LOW_BATTERY_CHOICES
                        .iter()
                        .position(|x| *x == s.low_battery_threshold);
                    if let Some(i) = w::segmented(ui, &p, "lowbat", &labels, sel, true, 30.0) {
                        self.shared
                            .update_settings(|s| s.low_battery_threshold = LOW_BATTERY_CHOICES[i]);
                    }
                },
            );
        });
        card(ui, &p, |ui| {
            section_title(ui, &p, ph::BLUETOOTH, "Connection");
            ui.horizontal(|ui| {
                if w::ghost_button(ui, &p, Some(ph::ARROWS_CLOCKWISE), "Reconnect").clicked() {
                    self.act(Action::Reconnect);
                }
                if w::ghost_button(ui, &p, Some(ph::GEAR_SIX), "Bluetooth settings").clicked() {
                    crate::platform::open_bluetooth_settings();
                }
            });
            if self.state.simulated {
                ui.label(
                    RichText::new("Running with a simulated headset (--sim).")
                        .font(regular(12.0))
                        .color(p.warn),
                );
            }
        });
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(format!("{} {}", crate::APP_NAME, env!("CARGO_PKG_VERSION"))).font(semibold(12.0)).color(p.text_dim));
            ui.add(
                egui::Label::new(
                    RichText::new("Unofficial companion for the Sennheiser MOMENTUM 4. Not affiliated with or endorsed by Sennheiser or Sonova.")
                        .font(regular(11.5))
                        .color(p.text_faint),
                )
                .wrap(),
            );
        });
    }
}

fn preset_chip(
    ui: &mut Ui,
    p: &Palette,
    text: &str,
    selected: bool,
    enabled: bool,
) -> egui::Response {
    let font = if selected {
        semibold(12.5)
    } else {
        regular(12.5)
    };
    let color = if selected { p.on_accent } else { p.text };
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let (r, resp) = ui.allocate_exact_size(
        galley.size() + vec2(22.0, 12.0),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let fill = if selected {
        p.accent
    } else if resp.hovered() && enabled {
        p.surface2.gamma_multiply(1.4)
    } else {
        p.surface2
    };
    let alpha = if enabled || selected { 1.0 } else { 0.45 };
    ui.painter()
        .rect_filled(r, CornerRadius::same(14), fill.gamma_multiply(alpha));
    ui.painter()
        .galley(r.center() - galley.size() / 2.0, galley, color);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

fn hint(ui: &mut Ui, p: &Palette, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).font(regular(12.0)).color(p.text_faint)).wrap());
}

/// Clickable section row with a caret; returns true when clicked.
fn expander(ui: &mut Ui, p: &Palette, glyph: &str, title: &str, open: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    let ir = egui::Rect::from_min_size(pos2(r.left(), r.center().y - 15.0), vec2(30.0, 30.0));
    ui.painter()
        .rect_filled(ir, CornerRadius::same(9), p.surface2);
    ui.painter().text(
        ir.center(),
        Align2::CENTER_CENTER,
        glyph,
        icon(16.0),
        p.text_dim,
    );
    ui.painter().text(
        pos2(ir.right() + 12.0, r.center().y),
        Align2::LEFT_CENTER,
        title,
        regular(14.0),
        p.text,
    );
    ui.painter().text(
        pos2(r.right(), r.center().y),
        Align2::RIGHT_CENTER,
        if open { ph::CARET_UP } else { ph::CARET_DOWN },
        icon(15.0),
        p.text_dim,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// MOMENTUM 4 touch gestures (fixed; Smart Control Plus `TouchControls` config for the M4, fw ≥ 2.13.18).
const GESTURES: [(&str, &[(&str, &str)]); 3] = [
    (
        "Music",
        &[
            ("Tap", "Play / pause"),
            ("Double tap", "Transparent hearing on / off"),
            ("Swipe forward", "Next track"),
            ("Swipe back", "Previous track"),
            ("Swipe forward and hold", "Fast forward"),
            ("Swipe back and hold", "Rewind"),
        ],
    ),
    (
        "Anytime",
        &[
            ("Swipe up", "Volume up"),
            ("Swipe down", "Volume down"),
            ("Spread two fingers", "More transparency"),
            ("Pinch two fingers", "Less transparency"),
        ],
    ),
    (
        "Calls",
        &[
            ("Swipe forward", "Accept call"),
            ("Tap and hold", "Reject call"),
            ("Swipe back", "End call"),
            ("Tap", "Accept a second call"),
            ("Double tap", "Switch between calls"),
        ],
    ),
];

fn gesture_reference(ui: &mut Ui, p: &Palette) {
    egui::Frame::new()
        .fill(p.surface2)
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 3.0;
            for (gi, (group, rows)) in GESTURES.iter().enumerate() {
                if gi > 0 {
                    ui.add_space(6.0);
                }
                ui.label(RichText::new(*group).font(semibold(12.5)).color(p.text_dim));
                for (gesture, action) in rows.iter() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(*gesture).font(regular(12.5)).color(p.text));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(*action).font(regular(12.5)).color(p.text_dim));
                        });
                    });
                }
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new("The MOMENTUM 4 gestures are fixed and can't be reassigned.")
                    .font(regular(11.5))
                    .color(p.text_faint),
            );
        });
}

/// Inline confirmation panel with a title, explanation and action buttons.
fn confirm_box(
    ui: &mut Ui,
    p: &Palette,
    glyph: &str,
    title: &str,
    body: &str,
    buttons: impl FnOnce(&mut Ui),
) {
    egui::Frame::new()
        .fill(p.surface2)
        .stroke(egui::Stroke::new(1.0, p.warn.gamma_multiply(0.45)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(glyph).font(icon(16.0)).color(p.warn));
                ui.label(RichText::new(title).font(semibold(14.0)).color(p.text));
            });
            ui.add(
                egui::Label::new(RichText::new(body).font(regular(12.5)).color(p.text_dim)).wrap(),
            );
            ui.add_space(4.0);
            ui.horizontal(buttons);
        });
}

fn danger_button(ui: &mut Ui, p: &Palette, text: &str) -> egui::Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), semibold(14.0), egui::Color32::WHITE);
    let (r, resp) = ui.allocate_exact_size(
        galley.size() + vec2(32.0, 36.0 - galley.size().y),
        Sense::click(),
    );
    let fill = if resp.hovered() {
        w::lerp_color(p.bad, egui::Color32::WHITE, 0.08)
    } else {
        p.bad
    };
    ui.painter().rect_filled(r, CornerRadius::same(10), fill);
    ui.painter().galley(
        r.center() - galley.size() / 2.0,
        galley,
        egui::Color32::WHITE,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Small trash-can icon button.
fn forget_button(ui: &mut Ui, p: &Palette) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(r, CornerRadius::same(9), p.surface2.gamma_multiply(1.3));
    }
    ui.painter().text(
        r.center(),
        Align2::CENTER_CENTER,
        ph::TRASH,
        icon(16.0),
        if resp.hovered() { p.bad } else { p.text_dim },
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Forget device")
}
