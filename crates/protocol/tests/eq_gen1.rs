//! Golden vectors produced by running the official app's own EQ code (`re/tools/eq_gen1_vectors.mjs`, kept
//! local because it copies that code; see PROTOCOL.md §0).

use protocol::eq::{M4_GEN1, gen1_encode};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Vector {
    gains: [f64; 5],
    bass: bool,
    level_shift_x10: i8,
    knob_y: u16,
    params: Vec<i16>,
}

#[test]
fn matches_official_app_byte_for_byte() {
    let vectors: Vec<Vector> = serde_json::from_str(include_str!("eq_gen1_vectors.json")).unwrap();
    assert!(vectors.len() >= 30);
    for v in vectors {
        let e = gen1_encode(&M4_GEN1, &v.gains, v.bass);
        assert_eq!(
            e.params, v.params,
            "params for {:?} bass={}",
            v.gains, v.bass
        );
        assert_eq!(
            e.level_shift_x10, v.level_shift_x10,
            "level shift for {:?} bass={}",
            v.gains, v.bass
        );
        assert_eq!(e.knob_y, v.knob_y, "knob y for bass={}", v.bass);
    }
}
