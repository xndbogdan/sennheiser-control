//! Design tokens, fonts and egui style.

use std::sync::Arc;

use egui::epaint::Shadow;
use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke,
    TextStyle, Visuals,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub surface: Color32,
    pub surface2: Color32,
    pub border: Color32,
    /// Inactive track of toggles, sliders and gauges.
    pub track: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    pub accent2: Color32,
    pub on_accent: Color32,
    pub good: Color32,
    pub warn: Color32,
    pub bad: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

impl Palette {
    pub const DARK: Palette = Palette {
        dark: true,
        bg: hex(0x0E1014),
        surface: hex(0x171A21),
        surface2: hex(0x22262F),
        border: hex(0x272C37),
        track: hex(0x363C49),
        text: hex(0xECEEF3),
        text_dim: hex(0x9AA1B2),
        text_faint: hex(0x646B7D),
        accent: hex(0x5B8CFF),
        accent2: hex(0x8A5CFF),
        on_accent: hex(0xFFFFFF),
        good: hex(0x3CCB7F),
        warn: hex(0xF5A524),
        bad: hex(0xF04F5A),
    };
    pub const LIGHT: Palette = Palette {
        dark: false,
        bg: hex(0xF2F3F7),
        surface: hex(0xFFFFFF),
        surface2: hex(0xEBEDF2),
        border: hex(0xE1E4EB),
        track: hex(0xCDD2DB),
        text: hex(0x15171D),
        text_dim: hex(0x5D6375),
        text_faint: hex(0x9298A8),
        accent: hex(0x3F6FF2),
        accent2: hex(0x7A4DF0),
        on_accent: hex(0xFFFFFF),
        good: hex(0x22A564),
        warn: hex(0xD98A06),
        bad: hex(0xE0414D),
    };

    pub fn accent_soft(&self) -> Color32 {
        self.accent
            .gamma_multiply(if self.dark { 0.18 } else { 0.12 })
    }

    pub fn level(&self, percent: u8) -> Color32 {
        match percent {
            0..=15 => self.bad,
            16..=40 => self.warn,
            _ => self.good,
        }
    }
}

pub const SEMIBOLD: &str = "semibold";
pub const BOLD: &str = "bold";

pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD.into()))
}

pub fn icon(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("phosphor".into()))
}

fn system_font(file: &str) -> Option<Vec<u8>> {
    let dir = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "C:\\Windows".into());
    std::fs::read(dir.join("Fonts").join(file)).ok()
}

/// Segoe UI from the system (not bundled), phosphor icons as fallback glyphs.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    let mut add = |name: &str, file: &str, family: FontFamily| {
        if let Some(bytes) = system_font(file) {
            fonts
                .font_data
                .insert(name.to_owned(), Arc::new(FontData::from_owned(bytes)));
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, name.to_owned());
            true
        } else {
            false
        }
    };
    add("segoe", "segoeui.ttf", FontFamily::Proportional);
    let has_sb = add("segoe-sb", "seguisb.ttf", FontFamily::Name(SEMIBOLD.into()));
    let has_b = add("segoe-b", "segoeuib.ttf", FontFamily::Name(BOLD.into()));
    // fallbacks so the named families always resolve (icons + default fonts)
    let proportional = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (fam, has) in [(SEMIBOLD, has_sb), (BOLD, has_b)] {
        let list = fonts
            .families
            .entry(FontFamily::Name(fam.into()))
            .or_default();
        for f in &proportional {
            if !(list.contains(f) || has && f == "segoe") {
                list.push(f.clone());
            }
        }
    }
    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &egui::Context, p: &Palette) {
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, regular(12.0)),
            (TextStyle::Body, regular(14.0)),
            (TextStyle::Button, regular(14.0)),
            (TextStyle::Heading, semibold(20.0)),
            (
                TextStyle::Monospace,
                FontId::new(13.0, FontFamily::Monospace),
            ),
        ]
        .into();
        let sp = &mut style.spacing;
        sp.item_spacing = egui::vec2(8.0, 8.0);
        sp.button_padding = egui::vec2(12.0, 6.0);
        sp.interact_size = egui::vec2(36.0, 28.0);
        sp.window_margin = Margin::same(12);
        sp.scroll.bar_width = 6.0;
        sp.scroll.floating = true;

        let mut v = if p.dark {
            Visuals::dark()
        } else {
            Visuals::light()
        };
        v.panel_fill = p.bg;
        v.window_fill = p.surface;
        v.window_stroke = Stroke::new(1.0, p.border);
        v.window_corner_radius = CornerRadius::same(14);
        v.window_shadow = Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(if p.dark { 90 } else { 40 }),
        };
        v.popup_shadow = v.window_shadow;
        v.extreme_bg_color = p.surface2;
        v.faint_bg_color = p.surface2;
        v.override_text_color = Some(p.text);
        v.hyperlink_color = p.accent;
        v.selection.bg_fill = p.accent.gamma_multiply(0.45);
        v.selection.stroke = Stroke::new(1.0, p.accent);
        v.text_cursor.stroke = Stroke::new(2.0, p.accent);
        let w = &mut v.widgets;
        let radius = CornerRadius::same(10);
        w.noninteractive.bg_fill = p.surface;
        w.noninteractive.weak_bg_fill = p.surface;
        w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        w.noninteractive.fg_stroke = Stroke::new(1.0, p.text_dim);
        w.noninteractive.corner_radius = radius;
        for (state, fill) in [
            (&mut w.inactive, p.surface2),
            (&mut w.hovered, p.surface2.gamma_multiply(1.25)),
            (&mut w.active, p.accent_soft()),
            (&mut w.open, p.surface2),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.bg_stroke = Stroke::NONE;
            state.fg_stroke = Stroke::new(1.0, p.text);
            state.corner_radius = radius;
            state.expansion = 0.0;
        }
        w.hovered.bg_stroke = Stroke::new(1.0, p.border);
        style.visuals = v;
    });
}
