//! Golden regression pin (Day 8): the reference design's summary is frozen
//! in tests/fixtures/reference_summary.json. Any physics, hashing, or
//! serialization change that moves these numbers fails here first — on
//! purpose. If the change is intentional, re-freeze with:
//!   cargo test -p ascent-app print_reference_summary -- --ignored --nocapture

use ascent_app::{run_design, Design};
use serde_json::Value;

const GOLDEN: &str = include_str!("fixtures/reference_summary.json");

#[test]
#[ignore]
fn print_reference_summary() {
    let r = run_design(&Design::reference()).unwrap();
    println!("{}", serde_json::to_string_pretty(&r.summary).unwrap());
}

#[test]
fn reference_summary_matches_golden_pin_exactly() {
    let golden: Value = serde_json::from_str(GOLDEN).expect("golden fixture must parse");
    let live: Value =
        serde_json::to_value(run_design(&Design::reference()).unwrap().summary).unwrap();

    assert_eq!(
        live["input_hash"], golden["input_hash"],
        "input hash drifted — the canonical sim input changed"
    );
    for field in [
        "apogee_m",
        "apogee_time_s",
        "max_velocity_ms",
        "burnout_time_s",
        "burnout_velocity_ms",
        "rail_exit_velocity_ms",
        "landing_time_s",
        "landing_velocity_ms",
    ] {
        let (l, g) = (live[field].as_f64().unwrap(), golden[field].as_f64().unwrap());
        assert!(
            (l - g).abs() < 1e-9,
            "{field} drifted: live {l} vs golden {g}"
        );
    }
}
