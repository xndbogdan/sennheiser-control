//! Main window.

pub mod eq;
pub mod theme;
mod views;
pub mod widgets;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use egui::{Align, Align2, CornerRadius, Layout, Sense, UiBuilder, ViewportCommand, pos2, vec2};
use egui_phosphor::regular as ph;
use engine::{Action, AppState, LinkState};

use crate::settings::{Settings, ThemePref};
use crate::shared::{Shared, UiCmd};
use theme::Palette;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Main,
    Settings,
}

/// A value the user is editing locally; shown instead of the device value until the device catches up.
struct Pending<T> {
    value: T,
    since: Instant,
    dragging: bool,
}

/// A multipoint connect/disconnect the user started; shown as in progress until the headset confirms.
#[derive(Clone, Copy)]
struct HostPending {
    index: u8,
    connect: bool,
    since: Instant,
}

const HOST_PENDING_TIMEOUT: Duration = Duration::from_secs(12);
const SEND_INTERVAL: Duration = Duration::from_millis(80);
const PENDING_GRACE: Duration = Duration::from_millis(1500);

pub struct HeadsetApp {
    shared: Arc<Shared>,
    rx: Receiver<UiCmd>,
    tray_thread: Option<JoinHandle<()>>,
    visible: bool,
    quitting: bool,
    pub page: Page,
    state: AppState,
    state_version: u64,
    settings: Settings,
    settings_version: u64,
    palette: Palette,
    autostart: bool,
    transparency: Option<Pending<f32>>,
    eq_bands: Option<Pending<[f32; protocol::eq::BANDS]>>,
    last_send: Instant,
    new_preset_name: Option<String>,
    pub show_devices: bool,
    /// Dev aid: initial scroll offset (applied once).
    pub dev_scroll: Option<f32>,
    show_about: bool,
    show_gestures: bool,
    /// Title-bar logo: (pixel size, texture).
    logo: Option<(u32, egui::TextureHandle)>,
    host_pending: Option<HostPending>,
    /// Paired-list index whose "Forget device" confirmation is showing.
    confirm_forget: Option<u8>,
    /// Hi-Res enable warning is showing.
    confirm_hi_res: bool,
    /// Factory-reset confirmation is showing.
    confirm_reset: bool,
    user_moved: bool,
    needs_position: bool,
    /// When the window last lost keyboard focus (clicking the tray icon steals focus before we hear about it).
    focus_lost_at: Option<Instant>,
    was_focused: bool,
}

impl HeadsetApp {
    pub fn new(
        shared: Arc<Shared>,
        rx: Receiver<UiCmd>,
        tray_thread: JoinHandle<()>,
        visible: bool,
    ) -> Self {
        let settings = shared.settings();
        Self {
            state: shared.engine.snapshot(),
            state_version: u64::MAX,
            settings_version: shared.settings_version(),
            palette: Palette::DARK,
            autostart: crate::platform::autostart_enabled(),
            shared,
            rx,
            tray_thread: Some(tray_thread),
            visible,
            quitting: false,
            page: Page::Main,
            settings,
            transparency: None,
            eq_bands: None,
            last_send: Instant::now(),
            new_preset_name: None,
            show_devices: false,
            dev_scroll: None,
            show_about: false,
            show_gestures: false,
            logo: None,
            host_pending: None,
            confirm_forget: None,
            confirm_hi_res: false,
            confirm_reset: false,
            user_moved: false,
            needs_position: true,
            focus_lost_at: None,
            was_focused: false,
        }
    }

    /// Dev aid: open every collapsible panel and confirmation for screenshots.
    pub fn open_dev_panels(&mut self) {
        self.show_about = true;
        self.show_gestures = true;
        self.confirm_hi_res = true;
        self.confirm_reset = true;
        // the simulator's idle tablet
        self.confirm_forget = Some(2);
    }

    fn act(&self, a: Action) {
        self.shared.dispatch(a);
    }

    fn set_visible(&mut self, ctx: &egui::Context, v: bool) {
        self.visible = v;
        if v {
            if !self.user_moved {
                self.needs_position = true;
            }
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
            self.autostart = crate::platform::autostart_enabled();
            self.act(Action::Refresh);
        } else {
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            self.show_devices = false;
        }
    }

    /// Place the window above the notification area, like a tray flyout.
    fn position_near_tray(&mut self, ctx: &egui::Context) {
        let Some((_, _, right, bottom)) = crate::platform::work_area() else {
            return;
        };
        let (ppp, outer) = ctx.input(|i| {
            (
                i.viewport().native_pixels_per_point,
                i.viewport().outer_rect,
            )
        });
        let (Some(ppp), Some(outer)) = (ppp, outer) else {
            return;
        };
        let margin = 12.0;
        let pos = pos2(
            right as f32 / ppp - outer.width() - margin,
            bottom as f32 / ppp - outer.height() - margin,
        );
        ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
        self.needs_position = false;
    }

    fn quit(&mut self, ctx: &egui::Context) {
        self.quitting = true;
        self.shared.quitting.store(true, Ordering::SeqCst);
        self.shared.tray_wake.signal();
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    fn sync(&mut self, ctx: &egui::Context) {
        let v = self.shared.engine.version();
        if v != self.state_version {
            self.state_version = v;
            self.state = self.shared.engine.snapshot();
            // remember the headset we successfully talked to
            if self.state.link == LinkState::Connected
                && let Some(id) = &self.state.active
                && self.settings.preferred_device.as_ref() != Some(id)
            {
                let id = id.clone();
                self.shared
                    .update_settings(|s| s.preferred_device = Some(id));
            }
        }
        let sv = self.shared.settings_version();
        if sv != self.settings_version {
            self.settings_version = sv;
            self.settings = self.shared.settings();
        }
        let dark = match self.settings.theme {
            ThemePref::Dark => true,
            ThemePref::Light => false,
            ThemePref::System => !crate::platform::apps_are_light(),
        };
        let p = if dark { Palette::DARK } else { Palette::LIGHT };
        if p != self.palette || self.state_version == 0 {
            self.palette = p;
            theme::apply(ctx, &p);
        }
        // drop local edits once the device caught up or the grace period expired
        if let Some(t) = &self.transparency
            && !t.dragging
            && (t.since.elapsed() > PENDING_GRACE
                || self.state.live().and_then(|h| h.transparency) == Some(t.value.round() as u8))
        {
            self.transparency = None;
        }
        if let Some(e) = &self.eq_bands
            && !e.dragging
            && (e.since.elapsed() > PENDING_GRACE
                || self.state.live().is_some_and(|h| {
                    h.eq.bands
                        .iter()
                        .zip(e.value)
                        .all(|(a, b)| (a - b).abs() < 0.051)
                }))
        {
            self.eq_bands = None;
        }
    }

    /// The app icon rendered at the exact pixel size for `points` on this display (re-rendered on DPI change).
    fn logo_texture(&mut self, ctx: &egui::Context, points: f32) -> egui::TextureId {
        let px = (points * ctx.pixels_per_point()).round().max(1.0) as u32;
        if self.logo.as_ref().is_none_or(|(size, _)| *size != px) {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [px as usize, px as usize],
                &crate::tray::icon::app_icon(px),
            );
            let tex = ctx.load_texture("app-logo", image, egui::TextureOptions::LINEAR);
            self.logo = Some((px, tex));
        }
        self.logo
            .as_ref()
            .map(|(_, t)| t.id())
            .expect("logo texture")
    }

    fn throttled(&mut self, force: bool) -> bool {
        if force || self.last_send.elapsed() >= SEND_INTERVAL {
            self.last_send = Instant::now();
            true
        } else {
            false
        }
    }

    fn title_bar(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        let ctx = ui.ctx().clone();
        let (bar, resp) =
            ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click_and_drag());
        if resp.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            self.user_moved = true;
        }
        let painter = ui.painter();
        let icon_rect =
            egui::Rect::from_center_size(pos2(bar.left() + 28.0, bar.center().y), vec2(24.0, 24.0));
        let logo = self.logo_texture(&ctx, 24.0);
        painter.image(
            logo,
            icon_rect,
            egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        painter.text(
            pos2(icon_rect.right() + 10.0, bar.center().y),
            Align2::LEFT_CENTER,
            crate::APP_NAME,
            theme::semibold(13.0),
            p.text_dim,
        );

        ui.scope_builder(
            UiBuilder::new()
                .max_rect(bar.shrink2(vec2(10.0, 0.0)))
                .layout(Layout::right_to_left(Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let close_tip = if self.settings.close_to_tray {
                    "Hide to tray"
                } else {
                    "Quit"
                };
                if widgets::title_button(ui, &p, ph::X, p.bad, close_tip).clicked() {
                    if self.settings.close_to_tray {
                        self.set_visible(&ctx, false);
                    } else {
                        self.quit(&ctx);
                    }
                }
                if widgets::title_button(ui, &p, ph::MINUS, p.surface2, "Minimize to tray")
                    .clicked()
                {
                    self.set_visible(&ctx, false);
                }
                let (glyph, tip) = if self.page == Page::Settings {
                    (ph::ARROW_LEFT, "Back")
                } else {
                    (ph::GEAR_SIX, "Settings")
                };
                if widgets::title_button(ui, &p, glyph, p.surface2, tip).clicked() {
                    self.page = if self.page == Page::Settings {
                        Page::Main
                    } else {
                        Page::Settings
                    };
                }
            },
        );
    }

    fn resize_grip(&self, ui: &mut egui::Ui) {
        let p = self.palette;
        let r = ui.max_rect();
        let grip = egui::Rect::from_min_size(r.max - vec2(16.0, 16.0), vec2(16.0, 16.0));
        let resp = ui.interact(grip, ui.id().with("grip"), Sense::drag());
        if resp.drag_started() {
            ui.ctx().send_viewport_cmd(ViewportCommand::BeginResize(
                egui::ResizeDirection::SouthEast,
            ));
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
        }
        let c = p.text_faint.gamma_multiply(0.6);
        for i in 0..3 {
            let o = 4.0 + i as f32 * 3.5;
            ui.painter().line_segment(
                [
                    pos2(grip.right() - 3.0, grip.bottom() - o),
                    pos2(grip.right() - o, grip.bottom() - 3.0),
                ],
                egui::Stroke::new(1.0, c),
            );
        }
    }

    fn toast(&self, ctx: &egui::Context) {
        let p = self.palette;
        let Some(msg) = &self.state.last_error else {
            return;
        };
        egui::Area::new(egui::Id::new("toast"))
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -18.0))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(if p.dark {
                        egui::Color32::from_rgb(0x2A, 0x1C, 0x20)
                    } else {
                        egui::Color32::from_rgb(0xFF, 0xEE, 0xEF)
                    })
                    .stroke(egui::Stroke::new(1.0, p.bad.gamma_multiply(0.5)))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.set_max_width(340.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(ph::WARNING_CIRCLE)
                                    .font(theme::icon(16.0))
                                    .color(p.bad),
                            );
                            ui.label(
                                egui::RichText::new(msg)
                                    .font(theme::regular(13.0))
                                    .color(p.text),
                            );
                        });
                    });
            });
    }
}

impl eframe::App for HeadsetApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                UiCmd::Show => self.set_visible(ctx, true),
                UiCmd::Toggle => {
                    // visible and in front → hide; visible but buried → bring forward; hidden → show
                    let in_front = self.was_focused
                        || self
                            .focus_lost_at
                            .is_some_and(|t| t.elapsed() < Duration::from_millis(500));
                    let show = !(self.visible && in_front);
                    self.set_visible(ctx, show);
                }
                UiCmd::OpenSettings => {
                    self.page = Page::Settings;
                    self.set_visible(ctx, true);
                }
                UiCmd::Quit => self.quit(ctx),
            }
        }
        let (close, minimized, focused) = ctx.input(|i| {
            let v = i.viewport();
            (v.close_requested(), v.minimized, v.focused)
        });
        let focused = focused.unwrap_or(false) && self.visible;
        if self.was_focused && !focused {
            self.focus_lost_at = Some(Instant::now());
        }
        self.was_focused = focused;
        if close && !self.quitting {
            if self.settings.close_to_tray {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                self.set_visible(ctx, false);
            } else {
                self.quitting = true;
                self.shared.quitting.store(true, Ordering::SeqCst);
                self.shared.tray_wake.signal();
            }
        }
        if minimized == Some(true) && self.visible {
            self.set_visible(ctx, false);
        }
        self.sync(ctx);
        if self.visible && self.needs_position {
            self.position_near_tray(ctx);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let p = self.palette;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.bg))
            .show(ui, |ui| {
                self.title_bar(ui);
                let mut scroll = egui::ScrollArea::vertical().auto_shrink([false; 2]);
                // wait until the page is fully populated, or the offset gets clamped to a short page
                let synced = self
                    .state
                    .live()
                    .is_some_and(|h| !h.paired_hosts.is_empty());
                if synced && let Some(y) = self.dev_scroll.take() {
                    scroll = scroll.vertical_scroll_offset(y);
                }
                scroll.show(ui, |ui| {
                    egui::Frame::new()
                        .inner_margin(egui::Margin {
                            left: 14,
                            right: 14,
                            top: 2,
                            bottom: 18,
                        })
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 12.0;
                            match self.page {
                                Page::Main => self.main_page(ui),
                                Page::Settings => self.settings_page(ui),
                            }
                        });
                });
                self.resize_grip(ui);
            });
        self.toast(ui.ctx());
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.palette.bg.to_normalized_gamma_f32()
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shared.quitting.store(true, Ordering::SeqCst);
        self.shared.tray_wake.signal();
        self.shared.engine.shutdown();
        if let Some(t) = self.tray_thread.take() {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !t.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}
