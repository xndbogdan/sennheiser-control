//! 5-band user EQ for the M4 (PROTOCOL.md §4.2).
//!
//! Gen1 (fw 2.12.0 ..< 3.18.0) renders the five UI gains as a master gain + four high-shelf filters and needs the
//! app's loudness/headroom math. Gen2 (fw >= 3.18.0) takes per-band slider gains in dB×10.
//! Gen1 arithmetic deliberately mirrors the JavaScript (`Math.round`, Int16/Int8 truncation) so the encoded bytes are
//! identical to the official app; see `tests/eq_gen1.rs`.

use crate::request::Request;

pub const BANDS: usize = 5;
pub const BAND_LABELS: [&str; BANDS] = ["63", "250", "1k", "4k", "8k"];
pub const MIN_GAIN_DB: f32 = -6.0;
pub const MAX_GAIN_DB: f32 = 6.0;

/// Built-in presets the official app ships for the M4 (UI gains in dB).
/// Smart Control Plus M4 presets (`products/m4` `geq_config.presets`). "Neutral" isn't a curve: it is audio
/// mode `Off`.
pub const PRESETS: &[(&str, [f32; BANDS])] = &[
    ("Rock", [0.0, 2.0, 2.5, 1.5, -2.0]),
    ("Pop", [0.0, -2.5, 0.0, 2.5, 0.0]),
    ("Dance", [3.5, 2.0, -1.5, 1.5, 3.0]),
    ("Hip-Hop", [3.0, 1.5, -1.5, 0.0, -1.5]),
    ("Classical", [-2.0, -1.5, 0.0, 3.5, 4.0]),
    ("Movie", [0.0, 0.0, 2.0, 2.0, -2.0]),
    ("Jazz", [-3.2, 0.0, 2.2, 2.2, 0.0]),
];

pub fn clamp_gain(g: f32) -> f32 {
    if g.is_nan() {
        0.0
    } else {
        g.clamp(MIN_GAIN_DB, MAX_GAIN_DB)
    }
}

/// Snap to the 0.1 dB grid the headset works in.
pub fn snap(g: f32) -> f32 {
    (clamp_gain(g) * 10.0).round() / 10.0
}

// ---------------------------------------------------------------------------------------------------------------
// Gen 2

/// dB → device units (dB × 10, "centibel").
pub fn gen2_units(db: f32) -> i8 {
    (clamp_gain(db) * 10.0).round() as i8
}

pub fn gen2_db(units: i8) -> f32 {
    units as f32 / 10.0
}

/// Requests that write `gains` (only bands that differ from `current`) and the bass-boost sub-mode.
pub fn gen2_requests(
    current: Option<&[f32; BANDS]>,
    gains: &[f32; BANDS],
    bass_boost: Option<bool>,
) -> Vec<Request> {
    let mut out = Vec::new();
    if let Some(bb) = bass_boost {
        out.push(Request::SetEq2SubMode(u8::from(bb)));
    }
    for (band, &g) in gains.iter().enumerate() {
        let units = gen2_units(g);
        if current.is_none_or(|c| gen2_units(c[band]) != units) {
            out.push(Request::SetEq2Slider {
                band: band as u8,
                gain_x10: units,
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Gen 1

pub struct Gen1Params {
    pub min_gain: f64,
    pub max_gain: f64,
    pub bass_boost: [f64; BANDS],
    pub loudness_weights: [f64; BANDS],
    pub max_headroom: [f64; BANDS],
    pub q_factor: [f64; 4],
    /// Shelf corner frequencies for bands 1..=4 (Hz). Band 0 is the master gain.
    pub shelf_freq: [f64; 4],
}

const SQRT1_2: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// `get5BandParametersForDevice(M4)` (bundle.pretty.js:70218).
pub const M4_GEN1: Gen1Params = Gen1Params {
    min_gain: -6.0,
    max_gain: 6.0,
    bass_boost: [5.0, 5.0, 0.0, 0.0, 0.0],
    loudness_weights: [-26.683, -10.813, -4.773, -4.813, -5.993],
    max_headroom: [5.0, 5.0, 4.0, 3.0, 2.0],
    q_factor: [SQRT1_2; 4],
    shelf_freq: [120.0, 400.0, 3000.0, 6000.0],
};

/// Knob Y storage flags (`EqKnobValuesCoder`).
pub const KNOB_NEW_STORAGE: u16 = 0x4000;
pub const KNOB_OLD_CLASSIC: u16 = 0xFF00;
pub const KNOB_BASS_BOOST: u16 = 0x1000;
pub const KNOB_Y_INDEX: u16 = 1;
/// Filter type written for every shelf (inferred: Qualcomm "2nd-order high shelf").
pub const FILTER_HIGH_SHELF: i16 = 11;

#[derive(Debug, Clone, PartialEq)]
pub struct Gen1Config {
    pub level_shift: f64,
    pub master: f64,
    pub shelves: [f64; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Gen1Encoded {
    pub params: Vec<i16>,
    pub level_shift_x10: i8,
    pub knob_y: u16,
}

/// JavaScript `Math.round`: nearest, ties toward +∞.
fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

/// JavaScript ToInt16 (truncate, wrap).
fn js_i16(x: f64) -> i16 {
    if !x.is_finite() {
        return 0;
    }
    (x.trunc() as i64 as u16) as i16
}

fn js_i8(x: f64) -> i8 {
    if !x.is_finite() {
        return 0;
    }
    (x.trunc() as i64 as u8) as i8
}

pub fn gen1_config(p: &Gen1Params, gains: &[f64; BANDS], bass_boost: bool) -> Gen1Config {
    let g: Vec<f64> = gains
        .iter()
        .map(|&t| t.max(p.min_gain).min(p.max_gain))
        .collect();
    let o: Vec<f64> = if bass_boost {
        g.iter().zip(p.bass_boost).map(|(t, b)| t + b).collect()
    } else {
        g
    };
    let sum = o
        .iter()
        .zip(p.loudness_weights)
        .fold(0.0, |acc, (t, w)| acc + 10f64.powf((t + w) / 10.0));
    let s = -10.0 * sum.log10();
    let r = o
        .iter()
        .zip(p.max_headroom)
        .map(|(t, h)| t + s - h)
        .fold(0.0, f64::max);
    let clipped = if r > 0.0 { s - r } else { s };
    let c = js_round(10.0 * clipped) / 10.0;
    let shifted: Vec<f64> = o.iter().map(|t| t + c).collect();
    let mut d = [0.0; BANDS];
    d[0] = shifted[0];
    for k in 1..BANDS {
        d[k] = shifted[k] - shifted[k - 1];
    }
    if (100.0 * d[0]).trunc() == 425.0 {
        d[0] = 4.24;
    }
    Gen1Config {
        level_shift: c,
        master: d[0],
        shelves: [d[1], d[2], d[3], d[4]],
    }
}

pub fn gen1_params(p: &Gen1Params, cfg: &Gen1Config) -> Vec<i16> {
    let mut v = vec![17.0, 257.0, 60.0 * cfg.master];
    for band in 0..4 {
        let base = 272.0 + 16.0 * band as f64;
        v.extend([
            base,
            FILTER_HIGH_SHELF as f64,
            base + 1.0,
            3.0 * p.shelf_freq[band],
            base + 2.0,
            60.0 * cfg.shelves[band],
            base + 3.0,
            4096.0 * p.q_factor[band],
        ]);
    }
    v.push(0.0);
    v.into_iter().map(js_i16).collect()
}

pub fn gen1_encode(p: &Gen1Params, gains: &[f64; BANDS], bass_boost: bool) -> Gen1Encoded {
    let cfg = gen1_config(p, gains, bass_boost);
    Gen1Encoded {
        params: gen1_params(p, &cfg),
        level_shift_x10: js_i8(10.0 * cfg.level_shift),
        knob_y: KNOB_NEW_STORAGE | if bass_boost { KNOB_BASS_BOOST } else { 0 },
    }
}

/// The three writes the official app performs for a Gen1 EQ change, in order.
pub fn gen1_requests(p: &Gen1Params, gains: &[f32; BANDS], bass_boost: bool) -> Vec<Request> {
    let g = gains.map(|x| x as f64);
    let e = gen1_encode(p, &g, bass_boost);
    vec![
        Request::SetEq1Params(e.params),
        Request::SetEq1LevelShift(e.level_shift_x10),
        Request::SetEq1Knob {
            index: KNOB_Y_INDEX,
            value: e.knob_y,
        },
    ]
}

/// Query list for the master + shelf gains: `[5, 0x0101,0, 0x0112,0, 0x0122,0, 0x0132,0, 0x0142,0]`.
pub fn gen1_gain_query() -> Vec<i16> {
    vec![5, 257, 0, 274, 0, 290, 0, 306, 0, 322, 0]
}

/// Query list for the four filter types.
pub fn gen1_filter_type_query() -> Vec<i16> {
    vec![4, 272, 0, 288, 0, 304, 0, 320, 0]
}

/// Bass boost flag from knob Y; `None` when the knob holds legacy (3-band) storage.
pub fn gen1_knob_bass_boost(knob_y: u16) -> Option<bool> {
    if knob_y & KNOB_OLD_CLASSIC == KNOB_OLD_CLASSIC
        || knob_y & KNOB_NEW_STORAGE != KNOB_NEW_STORAGE
    {
        return None;
    }
    Some(knob_y & KNOB_BASS_BOOST != 0)
}

/// Filter-type response indicates an unconfigured EQ or bypassed bands 3/4 → the app re-sends a flat EQ.
pub fn gen1_needs_reset(filter_types: &[i16]) -> bool {
    // "unconfigured" (all four types 0) is a subset of "bands 3 and 4 bypassed"
    filter_types.len() == 9 && filter_types[6] == 0 && filter_types[8] == 0
}

/// Reverse of [`gen1_encode`] from the values read back (`Eq5BandGen1.decode`).
pub fn gen1_decode(
    p: &Gen1Params,
    level_shift_x10: i8,
    bass_boost: bool,
    gain_rsp: &[i16],
) -> Option<[f32; BANDS]> {
    if gain_rsp.len() < 11 {
        return None;
    }
    let level_shift = level_shift_x10 as f64 / 10.0;
    let o: Vec<f64> = (0..BANDS)
        .map(|t| gain_rsp[2 + 2 * t] as f64 / 60.0)
        .collect();
    let mut a = [0.0f64; BANDS];
    a[0] = o[0];
    for k in 1..BANDS {
        a[k] = o[k] + a[k - 1];
    }
    if bass_boost {
        for (v, b) in a.iter_mut().zip(p.bass_boost) {
            *v -= b;
        }
    }
    Some(a.map(|n| (js_round(10.0 * (n - level_shift)) / 10.0) as f32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_helpers_match_js() {
        assert_eq!(js_round(-12.5), -12.0);
        assert_eq!(js_round(12.5), 13.0);
        assert_eq!(js_round(-0.4), 0.0);
        assert_eq!(js_i16(2896.309), 2896);
        assert_eq!(js_i16(-89.99), -89);
        assert_eq!(js_i16(40000.0), -25536);
        assert_eq!(js_i8(-13.000000000000002), -13);
    }

    #[test]
    fn gen1_roundtrip_through_decode() {
        for (_, preset) in PRESETS {
            for bass in [false, true] {
                let g = preset.map(|x| x as f64);
                let e = gen1_encode(&M4_GEN1, &g, bass);
                let rsp: Vec<i16> = [
                    5,
                    257,
                    e.params[2],
                    274,
                    e.params[8],
                    290,
                    e.params[16],
                    306,
                    e.params[24],
                    322,
                    e.params[32],
                ]
                .to_vec();
                let back = gen1_decode(
                    &M4_GEN1,
                    e.level_shift_x10,
                    gen1_knob_bass_boost(e.knob_y).unwrap(),
                    &rsp,
                )
                .unwrap();
                for k in 0..BANDS {
                    assert!(
                        (back[k] - preset[k]).abs() < 0.11,
                        "{preset:?} bass={bass} -> {back:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn gen2_only_changed_bands() {
        let cur = [0.0, 1.0, 2.0, 3.0, 4.0];
        let req = gen2_requests(Some(&cur), &[0.0, 1.0, -2.0, 3.0, 4.04], None);
        assert_eq!(
            req,
            vec![Request::SetEq2Slider {
                band: 2,
                gain_x10: -20
            }]
        );
        let all = gen2_requests(None, &[0.29, 0.0, 0.0, 0.0, 9.0], Some(true));
        assert_eq!(all[0], Request::SetEq2SubMode(1));
        assert_eq!(
            all[1],
            Request::SetEq2Slider {
                band: 0,
                gain_x10: 3
            }
        );
        assert_eq!(
            all[5],
            Request::SetEq2Slider {
                band: 4,
                gain_x10: 60
            }
        );
    }
}
