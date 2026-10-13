//! 5-band EQ editor and preset helpers.

use egui::{Align2, Color32, CornerRadius, Mesh, Pos2, Rect, Sense, Shape, Stroke, Ui, pos2, vec2};
use protocol::eq::{BAND_LABELS, BANDS, PRESETS};

use super::theme::{Palette, regular, semibold};
use crate::settings::CustomPreset;

/// Name of the preset (built-in or custom) matching `bands`, if any.
pub fn matching_preset(bands: &[f32; BANDS], custom: &[CustomPreset]) -> Option<String> {
    let eq = |a: &[f32; BANDS]| a.iter().zip(bands).all(|(x, y)| (x - y).abs() < 0.051);
    PRESETS
        .iter()
        .find(|(_, b)| eq(b))
        .map(|(n, _)| n.to_string())
        .or_else(|| custom.iter().find(|p| eq(&p.bands)).map(|p| p.name.clone()))
}

/// What the EQ preset list shows as selected.
#[derive(Debug, Clone, PartialEq)]
pub enum EqChoice {
    /// Audio mode `Off`: the headset's own tuning.
    Neutral,
    /// A built-in or saved preset, by name.
    Preset(String),
    /// A curve that matches no preset.
    Custom,
}

impl EqChoice {
    pub fn label(&self) -> &str {
        match self {
            EqChoice::Neutral => "Neutral",
            EqChoice::Preset(n) => n,
            EqChoice::Custom => "Custom",
        }
    }
}

/// `bands` is what is on screen (a local edit, or the headset's curve). `None` while podcast mode replaces the EQ.
pub fn eq_choice(
    h: &engine::HeadsetState,
    bands: &[f32; BANDS],
    user: &[CustomPreset],
    editing: bool,
) -> Option<EqChoice> {
    if h.podcast() {
        return None;
    }
    if !editing && !h.eq_active() {
        return Some(EqChoice::Neutral);
    }
    Some(
        matching_preset(bands, user)
            .map(EqChoice::Preset)
            .unwrap_or(EqChoice::Custom),
    )
}

pub struct EqEdit {
    pub changed: bool,
    pub released: bool,
}

/// Catmull-Rom interpolation through the band points.
fn curve(points: &[Pos2], left: f32, right: f32) -> Vec<Pos2> {
    let mut pts = Vec::with_capacity(points.len() + 2);
    pts.push(pos2(left, points[0].y));
    pts.extend_from_slice(points);
    pts.push(pos2(right, points[points.len() - 1].y));
    let mut out = Vec::new();
    for i in 0..pts.len() - 1 {
        let p0 = pts[i.saturating_sub(1)];
        let p1 = pts[i];
        let p2 = pts[i + 1];
        let p3 = pts[(i + 2).min(pts.len() - 1)];
        let steps = 16;
        for s in 0..steps {
            let t = s as f32 / steps as f32;
            let t2 = t * t;
            let t3 = t2 * t;
            let f = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * ((2.0 * b)
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            out.push(pos2(f(p0.x, p1.x, p2.x, p3.x), f(p0.y, p1.y, p2.y, p3.y)));
        }
    }
    out.push(*pts.last().unwrap());
    out
}

pub fn editor(
    ui: &mut Ui,
    p: &Palette,
    bands: &mut [f32; BANDS],
    min: f32,
    max: f32,
    enabled: bool,
) -> EqEdit {
    let mut edit = EqEdit {
        changed: false,
        released: false,
    };
    let width = ui.available_width();
    let height = 196.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let plot = Rect::from_min_max(rect.min + vec2(30.0, 14.0), rect.max - vec2(8.0, 26.0));
    let alpha = if enabled { 1.0 } else { 0.35 };
    let painter = ui.painter_at(rect);
    let y_of = |g: f32| egui::remap(g, max..=min, plot.top()..=plot.bottom());
    let g_of = |y: f32| egui::remap(y, plot.top()..=plot.bottom(), max..=min);

    // grid
    for g in [max, max / 2.0, 0.0, min / 2.0, min] {
        let y = y_of(g);
        let c = if g == 0.0 {
            p.text_faint.gamma_multiply(0.6)
        } else {
            p.border
        };
        painter.line_segment(
            [pos2(plot.left(), y), pos2(plot.right(), y)],
            Stroke::new(1.0, c),
        );
        let label = if g > 0.0 {
            format!("+{g:.0}")
        } else {
            format!("{g:.0}")
        };
        painter.text(
            pos2(rect.left() + 22.0, y),
            Align2::RIGHT_CENTER,
            label,
            regular(11.0),
            p.text_faint,
        );
    }
    let xs: Vec<f32> = (0..BANDS)
        .map(|i| {
            egui::lerp(
                plot.left() + 18.0..=plot.right() - 18.0,
                i as f32 / (BANDS - 1) as f32,
            )
        })
        .collect();
    for (i, x) in xs.iter().enumerate() {
        painter.line_segment(
            [pos2(*x, plot.top()), pos2(*x, plot.bottom())],
            Stroke::new(1.0, p.border.gamma_multiply(0.6)),
        );
        painter.text(
            pos2(*x, plot.bottom() + 14.0),
            Align2::CENTER_CENTER,
            BAND_LABELS[i],
            regular(11.5),
            p.text_dim,
        );
    }

    // interaction (before drawing so handles reflect this frame's drag)
    let mut active = None;
    let fine = ui.input(|i| i.modifiers.shift);
    for (i, x) in xs.iter().enumerate() {
        let hit =
            Rect::from_center_size(pos2(*x, plot.center().y), vec2(44.0, plot.height() + 28.0));
        let resp = ui.interact(
            hit,
            ui.id().with(("eq-band", i)),
            if enabled {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            },
        );
        if !enabled {
            continue;
        }
        if resp.double_clicked() {
            bands[i] = 0.0;
            edit.changed = true;
            edit.released = true;
        } else if (resp.dragged() || resp.drag_started())
            && let Some(pos) = resp.interact_pointer_pos()
        {
            let step = if fine { 0.1 } else { 0.5 };
            let v = ((g_of(pos.y.clamp(plot.top(), plot.bottom())) / step).round() * step)
                .clamp(min, max);
            if (v - bands[i]).abs() > 0.001 {
                bands[i] = v;
                edit.changed = true;
            }
        }
        if resp.drag_stopped() {
            edit.released = true;
        }
        if resp.hovered() || resp.dragged() {
            active = Some(i);
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
    }

    // curve + gradient fill
    let pts: Vec<Pos2> = xs
        .iter()
        .zip(bands.iter())
        .map(|(x, g)| pos2(*x, y_of(*g)))
        .collect();
    let line = curve(&pts, plot.left(), plot.right());
    let zero = y_of(0.0);
    let mut mesh = Mesh::default();
    let top = p.accent.gamma_multiply(0.30 * alpha);
    let bottom = p.accent.gamma_multiply(0.02 * alpha);
    for w in line.windows(2) {
        let (a, b) = (w[0], w[1]);
        let base = mesh.vertices.len() as u32;
        let ca = if a.y < zero { top } else { bottom };
        let cb = if b.y < zero { top } else { bottom };
        mesh.colored_vertex(a, ca);
        mesh.colored_vertex(b, cb);
        mesh.colored_vertex(pos2(b.x, zero), p.accent.gamma_multiply(0.05 * alpha));
        mesh.colored_vertex(pos2(a.x, zero), p.accent.gamma_multiply(0.05 * alpha));
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    painter.add(Shape::mesh(mesh));
    painter.add(Shape::line(
        line,
        Stroke::new(2.5, p.accent.gamma_multiply(alpha)),
    ));

    for (i, pt) in pts.iter().enumerate() {
        let hot = active == Some(i);
        let r = if hot { 9.0 } else { 7.5 };
        painter.circle_filled(*pt + vec2(0.0, 1.0), r, Color32::from_black_alpha(50));
        painter.circle_filled(*pt, r, Color32::WHITE.gamma_multiply(alpha.max(0.6)));
        painter.circle_stroke(*pt, r, Stroke::new(2.5, p.accent.gamma_multiply(alpha)));
        if hot {
            let label = format!("{:+.1} dB", bands[i]);
            let galley = painter.layout_no_wrap(label, semibold(12.0), p.on_accent);
            let size = galley.size() + vec2(12.0, 6.0);
            let above = pt.y - 22.0 > plot.top();
            let center = if above {
                *pt - vec2(0.0, 22.0)
            } else {
                *pt + vec2(0.0, 22.0)
            };
            let bubble = Rect::from_center_size(center, size);
            painter.rect_filled(bubble, CornerRadius::same(6), p.accent);
            painter.galley(bubble.center() - galley.size() / 2.0, galley, p.on_accent);
        }
    }
    edit
}
