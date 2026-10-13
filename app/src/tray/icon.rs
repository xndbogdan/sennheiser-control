//! Tray icon and app icon rendering (tiny-skia shapes + ab_glyph text), at exact pixel sizes.

use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use tiny_skia::{Color, FillRule, LineCap, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayVisual {
    /// No supported headset connected.
    Disconnected,
    /// Headset connected to Windows, control link not up yet.
    Connecting,
    Battery {
        percent: Option<u8>,
        charging: bool,
    },
}

pub const GOOD: [u8; 3] = [0x3C, 0xCB, 0x7F];
pub const WARN: [u8; 3] = [0xF5, 0xA5, 0x24];
pub const BAD: [u8; 3] = [0xF0, 0x4F, 0x5A];
pub const ACCENT: [u8; 3] = [0x5B, 0x8C, 0xFF];

pub fn level_color(percent: u8) -> [u8; 3] {
    match percent {
        0..=15 => BAD,
        16..=40 => WARN,
        _ => GOOD,
    }
}

pub struct IconRenderer {
    font: Option<FontVec>,
}

fn load_font() -> Option<FontVec> {
    let dir = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "C:\\Windows".into());
    for name in ["segoeuib.ttf", "seguisb.ttf", "segoeui.ttf"] {
        if let Ok(bytes) = std::fs::read(dir.join("Fonts").join(name))
            && let Ok(f) = FontVec::try_from_vec(bytes)
        {
            return Some(f);
        }
    }
    None
}

fn rgba(c: [u8; 3], a: f32) -> Color {
    Color::from_rgba8(c[0], c[1], c[2], (a.clamp(0.0, 1.0) * 255.0) as u8)
}

fn paint(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p
}

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

/// Over-ear headphones glyph inside the square (ox, oy, s).
fn draw_headphones(pm: &mut Pixmap, ox: f32, oy: f32, s: f32, color: Color) {
    let stroke_w = (s * 0.115).max(1.4);
    let (cx, cy, r) = (ox + s * 0.5, oy + s * 0.56, s * 0.355);
    let mut pb = PathBuilder::new();
    // headband: upper half circle drawn as two quarter cubics
    let k = 0.552_284_8 * r;
    pb.move_to(cx - r, cy);
    pb.cubic_to(cx - r, cy - k, cx - k, cy - r, cx, cy - r);
    pb.cubic_to(cx + k, cy - r, cx + r, cy - k, cx + r, cy);
    if let Some(path) = pb.finish() {
        let stroke = Stroke {
            width: stroke_w,
            line_cap: LineCap::Round,
            ..Default::default()
        };
        pm.stroke_path(&path, &paint(color), &stroke, Transform::identity(), None);
    }
    let (cw, ch) = (s * 0.25, s * 0.40);
    let cup_y = oy + s * 0.50;
    for x in [cx - r - cw * 0.40, cx + r - cw * 0.60] {
        if let Some(p) = rounded_rect(x, cup_y, cw, ch, cw * 0.38) {
            pm.fill_path(
                &p,
                &paint(color),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
}

fn blend_px(pm: &mut Pixmap, x: i32, y: i32, c: [u8; 3], alpha: f32) {
    if x < 0 || y < 0 || x >= pm.width() as i32 || y >= pm.height() as i32 || alpha <= 0.0 {
        return;
    }
    let i = ((y as u32 * pm.width() + x as u32) * 4) as usize;
    let d = pm.data_mut();
    let a = alpha.min(1.0);
    for k in 0..3 {
        d[i + k] = (c[k] as f32 * a + d[i + k] as f32 * (1.0 - a)).round() as u8;
    }
    d[i + 3] = (255.0 * a + d[i + 3] as f32 * (1.0 - a)).round() as u8;
}

impl IconRenderer {
    pub fn new() -> Self {
        Self { font: load_font() }
    }

    /// Draw `text` centred in the box, auto-sized to fit.
    fn draw_text(&self, pm: &mut Pixmap, text: &str, area: (f32, f32, f32, f32), color: [u8; 3]) {
        let (bx, by, bw, bh) = area;
        let Some(font) = &self.font else { return };
        let layout = |px: f32| {
            let sf = font.as_scaled(PxScale::from(px));
            let mut x = 0.0;
            let mut out = Vec::new();
            let mut prev = None;
            for ch in text.chars() {
                let id = font.glyph_id(ch);
                if let Some(p) = prev {
                    x += sf.kern(p, id);
                }
                out.push(id.with_scale_and_position(px, point(x, 0.0)));
                x += sf.h_advance(id);
                prev = Some(id);
            }
            out
        };
        let bounds = |glyphs: &[ab_glyph::Glyph]| {
            let mut b: Option<ab_glyph::Rect> = None;
            for g in glyphs {
                if let Some(o) = font.outline_glyph(g.clone()) {
                    let r = o.px_bounds();
                    b = Some(match b {
                        None => r,
                        Some(a) => ab_glyph::Rect {
                            min: point(a.min.x.min(r.min.x), a.min.y.min(r.min.y)),
                            max: point(a.max.x.max(r.max.x), a.max.y.max(r.max.y)),
                        },
                    });
                }
            }
            b
        };
        let mut px = bh * 1.3;
        let Some(b) = bounds(&layout(px)) else { return };
        let fit = (bw / b.width()).min(bh / b.height());
        px *= fit;
        let glyphs = layout(px);
        let Some(b) = bounds(&glyphs) else { return };
        let dx = bx + (bw - b.width()) / 2.0 - b.min.x;
        let dy = by + (bh - b.height()) / 2.0 - b.min.y;
        for g in glyphs {
            if let Some(o) = font.outline_glyph(g) {
                let r = o.px_bounds();
                let (ox, oy) = ((r.min.x + dx).round() as i32, (r.min.y + dy).round() as i32);
                o.draw(|x, y, cov| blend_px(pm, ox + x as i32, oy + y as i32, color, cov));
            }
        }
    }

    /// RGBA (straight alpha) tray icon of `size`×`size` pixels.
    pub fn tray(&self, visual: TrayVisual, size: u32, light_taskbar: bool) -> Vec<u8> {
        let s = size as f32;
        let mut pm = Pixmap::new(size, size).expect("pixmap");
        let fg: [u8; 3] = if light_taskbar {
            [0x1B, 0x1D, 0x22]
        } else {
            [0xFF, 0xFF, 0xFF]
        };
        match visual {
            TrayVisual::Disconnected => draw_headphones(&mut pm, 0.0, 0.0, s, rgba(fg, 0.92)),
            TrayVisual::Connecting => {
                draw_headphones(&mut pm, 0.0, 0.0, s, rgba(fg, 0.45));
                let d = (s * 0.30).max(4.0);
                if let Some(p) = PathBuilder::from_circle(s - d / 2.0, s - d / 2.0, d / 2.0) {
                    pm.fill_path(
                        &p,
                        &paint(rgba(ACCENT, 1.0)),
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
            TrayVisual::Battery { percent: None, .. } => {
                draw_headphones(&mut pm, 0.0, 0.0, s, rgba(fg, 0.92))
            }
            TrayVisual::Battery {
                percent: Some(p),
                charging,
            } => {
                let bar_h = (s * 0.14).round().max(2.0);
                let gap = (s * 0.06).round().max(1.0);
                let text_h = s - bar_h - gap;
                let color = if charging {
                    GOOD
                } else if p <= 15 {
                    BAD
                } else {
                    fg
                };
                self.draw_text(&mut pm, &p.to_string(), (0.0, 0.0, s, text_h), color);
                let bar_y = s - bar_h;
                let inset = (s * 0.06).round();
                let track = Rect::from_xywh(inset, bar_y, s - 2.0 * inset, bar_h);
                if let Some(t) = track
                    .and_then(|r| rounded_rect(r.x(), r.y(), r.width(), r.height(), bar_h / 2.0))
                {
                    pm.fill_path(
                        &t,
                        &paint(rgba(fg, 0.30)),
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
                let fill_w = ((s - 2.0 * inset) * p.min(100) as f32 / 100.0).max(bar_h);
                let fill_c = if charging { GOOD } else { level_color(p) };
                if let Some(f) = rounded_rect(inset, bar_y, fill_w, bar_h, bar_h / 2.0) {
                    pm.fill_path(
                        &f,
                        &paint(rgba(fill_c, 1.0)),
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
        }
        to_straight(pm)
    }
}

/// The Sennheiser mark (`assets/sennheiser-logo.svg`): a 24×18 rectangle with the two "S" curves cut out.
const LOGO_SVG: &str = include_str!("../../../assets/sennheiser-logo.svg");

/// App icon (window, taskbar, exe, title bar): the Sennheiser mark's curves, white on a black rounded square.
pub fn app_icon(size: u32) -> Vec<u8> {
    let s = size as f32;
    let mut pm = Pixmap::new(size, size).expect("pixmap");
    if let Some(bg) = rounded_rect(0.0, 0.0, s, s, s * 0.22) {
        pm.fill_path(
            &bg,
            &paint(Color::BLACK),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    if let Some(mark) = logo_path() {
        let b = mark.bounds();
        let k = s * 0.86 / b.width();
        let t = Transform::from_row(
            k,
            0.0,
            0.0,
            k,
            (s - b.width() * k) / 2.0 - b.left() * k,
            (s - b.height() * k) / 2.0 - b.top() * k,
        );
        // the cut-outs are where the mark doesn't cover its own rectangle; the rectangle itself merges with
        // the black tile. Stay a little inside its black border so the outer edge never shows as a white line.
        if let (Some(mut holes), Some(inner)) = (
            tiny_skia::Mask::new(size, size),
            Rect::from_ltrb(
                b.left() + 0.4,
                b.top() + 0.4,
                b.right() - 0.4,
                b.bottom() - 0.4,
            ),
        ) {
            holes.fill_path(&mark, FillRule::EvenOdd, true, t);
            holes.invert();
            pm.fill_rect(inner, &paint(Color::WHITE), t, Some(&holes));
        }
    }
    to_straight(pm)
}

fn logo_path() -> Option<tiny_skia::Path> {
    let d = LOGO_SVG.split(" d=\"").nth(1)?.split('"').next()?;
    parse_svg_path(d)
}

/// Minimal SVG path-data parser: M L H V C Z, absolute and relative, with implicit repeats and compact numbers
/// such as `-.321.703`. Enough for the logo; anything else returns `None`.
fn parse_svg_path(d: &str) -> Option<tiny_skia::Path> {
    let b = d.as_bytes();
    let mut i = 0;
    let skip = |i: &mut usize| {
        while *i < b.len() && (b[*i].is_ascii_whitespace() || b[*i] == b',') {
            *i += 1;
        }
    };
    let starts_number =
        |i: usize| i < b.len() && (b[i].is_ascii_digit() || matches!(b[i], b'.' | b'-' | b'+'));
    let number = |i: &mut usize| -> Option<f32> {
        skip(i);
        let start = *i;
        if *i < b.len() && matches!(b[*i], b'-' | b'+') {
            *i += 1;
        }
        let mut dot = false;
        while *i < b.len() {
            match b[*i] {
                c if c.is_ascii_digit() => *i += 1,
                b'.' if !dot => {
                    dot = true;
                    *i += 1;
                }
                b'e' | b'E' => {
                    *i += 1;
                    if *i < b.len() && matches!(b[*i], b'-' | b'+') {
                        *i += 1;
                    }
                    while *i < b.len() && b[*i].is_ascii_digit() {
                        *i += 1;
                    }
                    break;
                }
                _ => break,
            }
        }
        d.get(start..*i)?.parse().ok()
    };

    let mut pb = PathBuilder::new();
    let (mut cx, mut cy, mut sx, mut sy) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    loop {
        skip(&mut i);
        if i >= b.len() {
            break;
        }
        let cmd = b[i];
        i += 1;
        let rel = cmd.is_ascii_lowercase();
        let mut first = true;
        loop {
            skip(&mut i);
            if !first && !starts_number(i) {
                break;
            }
            match cmd.to_ascii_uppercase() {
                b'M' => {
                    let (x, y) = (number(&mut i)?, number(&mut i)?);
                    let (x, y) = if rel { (cx + x, cy + y) } else { (x, y) };
                    if first {
                        pb.move_to(x, y);
                        (sx, sy) = (x, y);
                    } else {
                        pb.line_to(x, y);
                    }
                    (cx, cy) = (x, y);
                }
                b'L' => {
                    let (x, y) = (number(&mut i)?, number(&mut i)?);
                    let (x, y) = if rel { (cx + x, cy + y) } else { (x, y) };
                    pb.line_to(x, y);
                    (cx, cy) = (x, y);
                }
                b'H' => {
                    let x = number(&mut i)? + if rel { cx } else { 0.0 };
                    pb.line_to(x, cy);
                    cx = x;
                }
                b'V' => {
                    let y = number(&mut i)? + if rel { cy } else { 0.0 };
                    pb.line_to(cx, y);
                    cy = y;
                }
                b'C' => {
                    let o = if rel { (cx, cy) } else { (0.0, 0.0) };
                    let mut v = [0.0f32; 6];
                    for n in &mut v {
                        *n = number(&mut i)?;
                    }
                    pb.cubic_to(
                        v[0] + o.0,
                        v[1] + o.1,
                        v[2] + o.0,
                        v[3] + o.1,
                        v[4] + o.0,
                        v[5] + o.1,
                    );
                    (cx, cy) = (v[4] + o.0, v[5] + o.1);
                }
                b'Z' => {
                    pb.close();
                    (cx, cy) = (sx, sy);
                    break;
                }
                _ => return None,
            }
            first = false;
        }
    }
    pb.finish()
}

fn to_straight(pm: Pixmap) -> Vec<u8> {
    let mut v = pm.take();
    for px in v.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    v
}

/// Dev aid (`--render-icons DIR`): preview sheet of every tray state + `app.ico` for the exe resource.
pub fn dev_render(dir: &std::path::Path) {
    let r = IconRenderer::new();
    let states = [
        TrayVisual::Disconnected,
        TrayVisual::Connecting,
        TrayVisual::Battery {
            percent: Some(100),
            charging: false,
        },
        TrayVisual::Battery {
            percent: Some(87),
            charging: false,
        },
        TrayVisual::Battery {
            percent: Some(34),
            charging: false,
        },
        TrayVisual::Battery {
            percent: Some(9),
            charging: false,
        },
        TrayVisual::Battery {
            percent: Some(56),
            charging: true,
        },
        TrayVisual::Battery {
            percent: None,
            charging: false,
        },
    ];
    let sizes = [16u32, 20, 24, 32];
    let scale = 6u32;
    let cell = 32 * scale + 12;
    for (light, name) in [(false, "tray-dark.png"), (true, "tray-light.png")] {
        let (w, h) = (cell * states.len() as u32, cell * sizes.len() as u32);
        let mut sheet = Pixmap::new(w, h).unwrap();
        sheet.fill(if light {
            Color::from_rgba8(0xF3, 0xF3, 0xF3, 255)
        } else {
            Color::from_rgba8(0x20, 0x20, 0x20, 255)
        });
        for (row, size) in sizes.iter().enumerate() {
            for (col, st) in states.iter().enumerate() {
                let px = r.tray(*st, *size, light);
                let k = scale * 32 / size;
                for y in 0..*size {
                    for x in 0..*size {
                        let i = ((y * size + x) * 4) as usize;
                        let a = px[i + 3] as f32 / 255.0;
                        if a == 0.0 {
                            continue;
                        }
                        for dy in 0..k {
                            for dx in 0..k {
                                let (sx, sy) = (
                                    col as u32 * cell + 6 + x * k + dx,
                                    row as u32 * cell + 6 + y * k + dy,
                                );
                                blend_px(
                                    &mut sheet,
                                    sx as i32,
                                    sy as i32,
                                    [px[i], px[i + 1], px[i + 2]],
                                    a,
                                );
                            }
                        }
                    }
                }
            }
        }
        let _ = sheet.save_png(dir.join(name));
    }
    let app = Pixmap::from_vec(
        app_icon(256),
        tiny_skia::IntSize::from_wh(256, 256).unwrap(),
    );
    if let Some(app) = app {
        let _ = app.save_png(dir.join("app-icon.png"));
    }
    let images: Vec<(u32, Vec<u8>)> = [16u32, 20, 24, 32, 40, 48, 64, 128, 256]
        .iter()
        .map(|s| (*s, app_icon(*s)))
        .collect();
    let _ = std::fs::write(dir.join("app.ico"), encode_ico(&images));
}

/// Encode RGBA images as a multi-size .ico (PNG-compressed entries are not needed; we use 32-bit BMP DIBs).
pub fn encode_ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    let mut blobs = Vec::new();
    for (size, rgba) in images {
        let s = *size;
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&(s as i32).to_le_bytes());
        dib.extend_from_slice(&((s * 2) as i32).to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&32u16.to_le_bytes());
        dib.extend_from_slice(&[0u8; 24]);
        for y in (0..s).rev() {
            for x in 0..s {
                let i = ((y * s + x) * 4) as usize;
                dib.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
            }
        }
        let mask_row = s.div_ceil(32) * 4;
        dib.extend(std::iter::repeat_n(0u8, (mask_row * s) as usize));
        let dim = if s >= 256 { 0 } else { s as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(dib.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += dib.len() as u32;
        blobs.push(dib);
    }
    for b in blobs {
        out.extend(b);
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn logo_svg_parses_to_the_mark() {
        let p = super::logo_path().expect("logo path");
        let b = p.bounds();
        // `M0 3v18h24V3z` is the mark's rectangle; the curves sit inside it
        assert_eq!(
            (b.left(), b.top(), b.right(), b.bottom()),
            (0.0, 3.0, 24.0, 21.0)
        );
        assert!(
            super::parse_svg_path("M0 0Q1 1 2 2").is_none(),
            "unsupported command"
        );
        let compact = super::parse_svg_path("M1 1c.5-.25.75.5 1 1z")
            .unwrap()
            .bounds();
        assert!((compact.right() - 2.0).abs() < 1e-6 && (compact.bottom() - 2.0).abs() < 1e-6);
    }
}
