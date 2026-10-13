//! Custom widgets drawn with the egui painter.

use std::f32::consts::{PI, TAU};

use egui::{
    Align, Align2, Color32, CornerRadius, Layout, Pos2, Rect, Response, Sense, Shape, Stroke, Ui,
    Vec2, pos2, vec2,
};
use egui_phosphor::regular as ph;

use super::theme::{Palette, bold, icon, regular, semibold};

const ANIM: f32 = 0.16;

pub fn card<R>(ui: &mut Ui, p: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn section_title(ui: &mut Ui, p: &Palette, glyph: &str, title: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(egui::RichText::new(glyph).font(icon(18.0)).color(p.accent));
        ui.label(
            egui::RichText::new(title)
                .font(semibold(15.0))
                .color(p.text),
        );
    });
    ui.add_space(4.0);
}

pub fn divider(ui: &mut Ui, p: &Palette) {
    ui.add_space(2.0);
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, p.border);
    ui.add_space(2.0);
}

/// iOS-style switch. Returns a response that is `changed()` when flipped.
pub fn toggle(ui: &mut Ui, p: &Palette, on: &mut bool, enabled: bool) -> Response {
    let size = vec2(40.0, 22.0);
    let (rect, mut resp) = ui.allocate_exact_size(
        size,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if enabled && resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(resp.id, *on, ANIM);
    let bg = lerp_color(p.track, p.accent, t);
    let alpha = if enabled { 1.0 } else { 0.4 };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(11), bg.gamma_multiply(alpha));
    let r = 8.0;
    let cx = egui::lerp((rect.left() + 3.0 + r)..=(rect.right() - 3.0 - r), t);
    painter.circle_filled(
        pos2(cx, rect.center().y + 0.5),
        r + 0.5,
        Color32::from_black_alpha(40),
    );
    painter.circle_filled(
        pos2(cx, rect.center().y),
        r,
        Color32::WHITE.gamma_multiply(alpha),
    );
    if enabled {
        resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Segmented control with a sliding highlight. Returns the clicked index.
pub fn segmented(
    ui: &mut Ui,
    p: &Palette,
    id_salt: &str,
    labels: &[&str],
    selected: Option<usize>,
    enabled: bool,
    height: f32,
) -> Option<usize> {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, height), Sense::hover());
    let id = ui.id().with(id_salt);
    let n = labels.len().max(1) as f32;
    let inner = rect.shrink(3.0);
    let seg_w = inner.width() / n;
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        CornerRadius::same((height / 2.0).min(12.0) as u8),
        p.surface2,
    );
    let alpha = if enabled { 1.0 } else { 0.45 };
    if let Some(sel) = selected {
        let x = ui
            .ctx()
            .animate_value_with_time(id.with("pill"), sel as f32, ANIM);
        let pill = Rect::from_min_size(
            pos2(inner.left() + x * seg_w, inner.top()),
            vec2(seg_w, inner.height()),
        );
        painter.rect_filled(
            pill,
            CornerRadius::same(((height - 6.0) / 2.0).min(10.0) as u8),
            p.accent.gamma_multiply(alpha),
        );
    }
    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(inner.left() + i as f32 * seg_w, inner.top()),
            vec2(seg_w, inner.height()),
        );
        let resp = ui.interact(
            r,
            id.with(i),
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        let is_sel = selected == Some(i);
        let color = if is_sel {
            p.on_accent
        } else if resp.hovered() && enabled {
            p.text
        } else {
            p.text_dim
        };
        let font = if is_sel {
            semibold(13.0)
        } else {
            regular(13.0)
        };
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            *label,
            font,
            color.gamma_multiply(alpha),
        );
        if enabled && resp.clicked() {
            clicked = Some(i);
        }
        if enabled {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
    }
    clicked
}

/// Two-sided slider over 0..=100 with a neutral centre (50): the fill grows from the centre towards the knob,
/// and the knob snaps to the centre when released close to it.
pub fn balance_slider(ui: &mut Ui, p: &Palette, value: &mut f32, enabled: bool) -> Response {
    const SNAP: f32 = 3.0;
    let h = 26.0;
    let w = ui.available_width();
    let (rect, mut resp) = ui.allocate_exact_size(
        vec2(w, h),
        if enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    );
    let track = Rect::from_center_size(rect.center(), vec2(rect.width() - 16.0, 6.0));
    if enabled
        && (resp.dragged() || resp.clicked())
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let mut v = (((pos.x - track.left()) / track.width()).clamp(0.0, 1.0) * 100.0).round();
        if (v - 50.0).abs() <= SNAP {
            v = 50.0;
        }
        if (v - *value).abs() > f32::EPSILON {
            *value = v;
            resp.mark_changed();
        }
    }
    let t = (*value / 100.0).clamp(0.0, 1.0);
    let alpha = if enabled { 1.0 } else { 0.4 };
    let painter = ui.painter();
    painter.rect_filled(track, CornerRadius::same(3), p.track);
    let centre = track.center().x;
    let knob_x = track.left() + t * track.width();
    let filled = Rect::from_min_max(
        pos2(centre.min(knob_x), track.min.y),
        pos2(centre.max(knob_x), track.max.y),
    );
    painter.rect_filled(
        filled,
        CornerRadius::same(3),
        p.accent.gamma_multiply(alpha),
    );
    // neutral mark
    painter.line_segment(
        [
            pos2(centre, track.top() - 5.0),
            pos2(centre, track.bottom() + 5.0),
        ],
        Stroke::new(2.0, p.text_faint.gamma_multiply(alpha)),
    );
    let knob = pos2(knob_x, track.center().y);
    let grow =
        ui.ctx()
            .animate_bool_with_time(resp.id.with("k"), resp.hovered() || resp.dragged(), ANIM);
    painter.circle_filled(
        knob + vec2(0.0, 1.0),
        9.0 + grow * 1.5,
        Color32::from_black_alpha(45),
    );
    painter.circle_filled(knob, 9.0 + grow * 1.5, Color32::WHITE.gamma_multiply(alpha));
    painter.circle_filled(knob, 3.5, p.accent.gamma_multiply(alpha));
    if enabled {
        resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Row: icon, title (+subtitle) on the left, trailing control on the right.
pub fn setting_row(
    ui: &mut Ui,
    p: &Palette,
    glyph: &str,
    title: &str,
    subtitle: Option<&str>,
    enabled: bool,
    trailing: impl FnOnce(&mut Ui),
) {
    let alpha = if enabled { 1.0 } else { 0.45 };
    ui.horizontal(|ui| {
        ui.set_min_height(if subtitle.is_some() { 40.0 } else { 30.0 });
        let (ir, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
        ui.painter()
            .rect_filled(ir, CornerRadius::same(9), p.surface2);
        ui.painter().text(
            ir.center(),
            Align2::CENTER_CENTER,
            glyph,
            icon(16.0),
            p.text_dim.gamma_multiply(alpha),
        );
        ui.add_space(4.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            trailing(ui);
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                if subtitle.is_some() {
                    ui.add_space(2.0);
                }
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(title)
                            .font(regular(14.0))
                            .color(p.text.gamma_multiply(alpha)),
                    )
                    .truncate(),
                );
                if let Some(s) = subtitle {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(s)
                                .font(regular(12.0))
                                .color(p.text_faint),
                        )
                        .wrap(),
                    );
                }
            });
        });
    });
}

/// Title + full-width control underneath (for segmented choices).
pub fn stacked_row(
    ui: &mut Ui,
    p: &Palette,
    glyph: &str,
    title: &str,
    value: Option<&str>,
    add: impl FnOnce(&mut Ui),
) {
    ui.horizontal(|ui| {
        let (ir, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
        ui.painter()
            .rect_filled(ir, CornerRadius::same(9), p.surface2);
        ui.painter().text(
            ir.center(),
            Align2::CENTER_CENTER,
            glyph,
            icon(16.0),
            p.text_dim,
        );
        ui.add_space(4.0);
        ui.label(egui::RichText::new(title).font(regular(14.0)).color(p.text));
        if let Some(v) = value {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(v)
                        .font(semibold(13.0))
                        .color(p.text_dim),
                );
            });
        }
    });
    ui.add_space(2.0);
    add(ui);
}

pub fn chip(ui: &mut Ui, p: &Palette, glyph: Option<&str>, text: &str) {
    let font = regular(12.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, p.text_dim);
    let icon_w = if glyph.is_some() { 16.0 } else { 0.0 };
    let size = vec2(galley.size().x + 16.0 + icon_w, 22.0);
    let (r, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(r, CornerRadius::same(11), p.surface2);
    let mut x = r.left() + 8.0;
    if let Some(g) = glyph {
        ui.painter().text(
            pos2(x + 6.0, r.center().y),
            Align2::CENTER_CENTER,
            g,
            icon(13.0),
            p.text_dim,
        );
        x += icon_w;
    }
    ui.painter().galley(
        pos2(x, r.center().y - galley.size().y / 2.0),
        galley,
        p.text_dim,
    );
}

pub fn status_dot(ui: &mut Ui, color: Color32) {
    let (r, _) = ui.allocate_exact_size(vec2(8.0, 14.0), Sense::hover());
    ui.painter().circle_filled(r.center(), 3.5, color);
}

/// Circular battery gauge.
pub fn battery_ring(
    ui: &mut Ui,
    p: &Palette,
    percent: Option<u8>,
    charging: bool,
    size: f32,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let c = rect.center();
    let radius = size / 2.0 - 4.0;
    let stroke_w = 5.0;
    let painter = ui.painter();
    painter.circle_stroke(c, radius, Stroke::new(stroke_w, p.track));
    if let Some(pct) = percent {
        let t = ui
            .ctx()
            .animate_value_with_time(resp.id.with("b"), pct as f32 / 100.0, 0.5);
        let color = if charging { p.good } else { p.level(pct) };
        arc(
            painter,
            c,
            radius,
            -PI / 2.0,
            -PI / 2.0 + TAU * t,
            Stroke::new(stroke_w, color),
        );
        painter.text(
            c + vec2(0.0, if charging { -3.0 } else { 0.0 }),
            Align2::CENTER_CENTER,
            format!("{pct}%"),
            bold(15.0),
            p.text,
        );
        if charging {
            painter.text(
                c + vec2(0.0, 12.0),
                Align2::CENTER_CENTER,
                ph::LIGHTNING,
                icon(11.0),
                p.good,
            );
        }
    } else {
        painter.text(c, Align2::CENTER_CENTER, "—", semibold(15.0), p.text_dim);
    }
    resp
}

pub fn arc(painter: &egui::Painter, c: Pos2, r: f32, a0: f32, a1: f32, stroke: Stroke) {
    let n = ((a1 - a0).abs() / TAU * 72.0).ceil().max(2.0) as usize;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    painter.add(Shape::line(pts.clone(), stroke));
    // round caps
    painter.circle_filled(pts[0], stroke.width / 2.0, stroke.color);
    painter.circle_filled(*pts.last().unwrap(), stroke.width / 2.0, stroke.color);
}

/// Over-ear headphones illustration.
pub fn headphones_art(painter: &egui::Painter, rect: Rect, p: &Palette, dim: bool) {
    let s = rect.width().min(rect.height());
    let c = rect.center();
    let alpha = if dim { 0.45 } else { 1.0 };
    // soft disc
    painter.circle_filled(c, s * 0.5, p.accent_soft().gamma_multiply(alpha));
    let band_r = s * 0.27;
    let band_c = c + vec2(0.0, s * 0.05);
    let band_col = lerp_color(p.text, p.accent, 0.35).gamma_multiply(alpha);
    arc(
        painter,
        band_c,
        band_r,
        PI,
        TAU,
        Stroke::new(s * 0.055, band_col),
    );
    let cup = vec2(s * 0.15, s * 0.26);
    for side in [-1.0f32, 1.0] {
        let cx = band_c.x + side * band_r;
        let r = Rect::from_center_size(pos2(cx, band_c.y + s * 0.06), cup);
        painter.rect_filled(
            r,
            CornerRadius::same((s * 0.06) as u8),
            lerp_color(p.accent, p.accent2, 0.5).gamma_multiply(alpha),
        );
        let pad = Rect::from_center_size(
            pos2(cx - side * cup.x * 0.55, r.center().y),
            vec2(cup.x * 0.35, cup.y * 0.78),
        );
        painter.rect_filled(pad, CornerRadius::same((s * 0.03) as u8), band_col);
    }
}

pub fn title_button(
    ui: &mut Ui,
    p: &Palette,
    glyph: &str,
    hover_fill: Color32,
    tooltip: &str,
) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(34.0, 30.0), Sense::click());
    let hovered = resp.hovered();
    if hovered {
        ui.painter()
            .rect_filled(r, CornerRadius::same(8), hover_fill);
    }
    let color = if hovered && hover_fill == p.bad {
        Color32::WHITE
    } else if hovered {
        p.text
    } else {
        p.text_dim
    };
    ui.painter()
        .text(r.center(), Align2::CENTER_CENTER, glyph, icon(16.0), color);
    resp.on_hover_text(tooltip)
}

pub fn primary_button(ui: &mut Ui, p: &Palette, glyph: Option<&str>, text: &str) -> Response {
    let font = semibold(14.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, p.on_accent);
    let iw = if glyph.is_some() { 22.0 } else { 0.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(galley.size().x + 32.0 + iw, 36.0), Sense::click());
    let fill = if resp.hovered() {
        lerp_color(p.accent, Color32::WHITE, 0.08)
    } else {
        p.accent
    };
    ui.painter().rect_filled(r, CornerRadius::same(10), fill);
    let mut x = r.left() + 16.0;
    if let Some(g) = glyph {
        ui.painter().text(
            pos2(x + 8.0, r.center().y),
            Align2::CENTER_CENTER,
            g,
            icon(16.0),
            p.on_accent,
        );
        x += iw;
    }
    ui.painter().galley(
        pos2(x, r.center().y - galley.size().y / 2.0),
        galley,
        p.on_accent,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn ghost_button(ui: &mut Ui, p: &Palette, glyph: Option<&str>, text: &str) -> Response {
    let font = regular(13.0);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, p.text);
    let iw = if glyph.is_some() { 20.0 } else { 0.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(galley.size().x + 24.0 + iw, 30.0), Sense::click());
    let fill = if resp.hovered() {
        p.surface2.gamma_multiply(1.3)
    } else {
        p.surface2
    };
    ui.painter().rect_filled(r, CornerRadius::same(9), fill);
    let mut x = r.left() + 12.0;
    if let Some(g) = glyph {
        ui.painter().text(
            pos2(x + 7.0, r.center().y),
            Align2::CENTER_CENTER,
            g,
            icon(14.0),
            p.text_dim,
        );
        x += iw;
    }
    ui.painter().galley(
        pos2(x, r.center().y - galley.size().y / 2.0),
        galley,
        p.text,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn spinner(ui: &mut Ui, p: &Palette, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let t = ui.input(|i| i.time) as f32;
    let a0 = t * 5.0;
    arc(
        ui.painter(),
        rect.center(),
        size / 2.0 - 2.0,
        a0,
        a0 + PI * 1.3,
        Stroke::new(2.5, p.accent),
    );
    ui.ctx().request_repaint();
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}
