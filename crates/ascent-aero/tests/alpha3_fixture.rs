//! Cross-check against the equation-exact Alpha III Barrowman fixture
//! (Codex task A2, `data/fixtures/alpha3_stability.json`), which is itself
//! cross-checked against OpenRocket's BarrowmanCalculatorTest at a pinned
//! revision.
//!
//! Ascent's v0.1 aero model is incompressible and uses the 0.466·L ogive CP
//! approximation; the fixture is evaluated at Mach 0.3 (β = 0.954) with the
//! exact tangent-ogive expression. Tolerances below bound exactly those two
//! known differences — anything looser is a real regression.

use ascent_aero::{fin_set_cn, nose_cn, total_cp_from_nose_m, BodyTube, FinSet, NoseCone, NoseShape, Vehicle};
use serde_json::Value;

const FIXTURE: &str = include_str!("../../../data/fixtures/alpha3_stability.json");

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).expect("alpha3_stability.json must parse")
}

fn f(v: &Value, path: &[&str]) -> f64 {
    let mut cur = v;
    for p in path {
        cur = &cur[*p];
    }
    cur.as_f64()
        .unwrap_or_else(|| panic!("fixture field {} missing or not a number", path.join(".")))
}

/// Build Ascent's Vehicle from the fixture geometry (masses are irrelevant
/// to CP, filled with fixture-neutral values).
fn fixture_vehicle(fx: &Value) -> Vehicle {
    let g = &fx["geometry"];
    Vehicle {
        name: "Alpha III (fixture geometry)".into(),
        nose: NoseCone {
            length_m: f(g, &["nose", "length_m"]),
            base_diameter_m: 2.0 * f(g, &["nose", "base_radius_m"]),
            mass_kg: 0.010,
        },
        body: BodyTube {
            length_m: f(g, &["body_tube", "length_m"]),
            outer_diameter_m: 2.0 * f(g, &["body_tube", "outer_radius_m"]),
            mass_kg: 0.015,
        },
        fins: FinSet {
            count: f(g, &["fin_set", "count"]) as u32,
            root_chord_m: f(g, &["fin_set", "root_chord_m"]),
            tip_chord_m: f(g, &["fin_set", "tip_chord_m"]),
            span_m: f(g, &["fin_set", "span_m"]),
            sweep_m: f(g, &["fin_set", "tip_leading_edge_sweep_m"]),
            position_from_nose_m: f(g, &["fin_set", "root_leading_edge_x_m"]),
            mass_kg: 0.003,
        },
        point_masses: vec![],
        motor_position_from_nose_m: 0.203,
    }
}

fn rel_err(ours: f64, reference: f64) -> f64 {
    ((ours - reference) / reference).abs()
}

#[test]
fn fin_planform_cp_matches_fixture_exactly() {
    let fx = fixture();
    let v = fixture_vehicle(&fx);
    let fins = fin_set_cn(&v);
    let expected = fx["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "fin_set")
        .unwrap()["cp_x_from_fin_root_leading_edge_m"]
        .as_f64()
        .unwrap();
    let ours_local = fins.cp_from_nose_m - v.fins.position_from_nose_m;
    // Same closed-form planform equation — must agree to float precision.
    assert!(
        (ours_local - expected).abs() < 1e-12,
        "fin local CP {ours_local} vs fixture {expected}"
    );
}

#[test]
fn fin_cn_alpha_within_compressibility_of_fixture() {
    let fx = fixture();
    let v = fixture_vehicle(&fx);
    let fins = fin_set_cn(&v);
    let expected = fx["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "fin_set")
        .unwrap()["cn_alpha_per_rad"]
        .as_f64()
        .unwrap();
    // Fixture is at Mach 0.3 (β = 0.954 boosts CNα ~4.8%); ours is
    // incompressible. 5% bounds the β effect with margin for nothing else.
    assert!(
        rel_err(fins.cn_alpha, expected) < 0.05,
        "fin CNα {} vs fixture {} ({}%)",
        fins.cn_alpha,
        expected,
        rel_err(fins.cn_alpha, expected) * 100.0
    );
}

#[test]
fn nose_cp_within_ogive_approximation_of_fixture() {
    let fx = fixture();
    let expected = fx["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "nose_cone")
        .unwrap()["cp_x_from_nose_m"]
        .as_f64()
        .unwrap();
    let ours = nose_cn(NoseShape::Ogive, f(&fixture(), &["geometry", "nose", "length_m"]));
    // 0.466·L vs exact tangent-ogive volume expression: ~0.8% apart.
    assert!(
        rel_err(ours.cp_from_nose_m, expected) < 0.015,
        "nose CP {} vs fixture {}",
        ours.cp_from_nose_m,
        expected
    );
    assert_eq!(ours.cn_alpha, 2.0);
}

#[test]
fn total_cp_within_half_percent_of_fixture() {
    let fx = fixture();
    let v = fixture_vehicle(&fx);
    let expected = f(&fx, &["totals", "cp_x_from_nose_m"]);
    let ours = total_cp_from_nose_m(&v, NoseShape::Ogive);
    // Fin-dominated total CP: the nose and compressibility deltas nearly
    // cancel in the weighted sum. 0.5% ≈ 1.1 mm on this airframe.
    assert!(
        rel_err(ours, expected) < 0.005,
        "total CP {ours} vs fixture {expected} ({}%)",
        rel_err(ours, expected) * 100.0
    );
    // And against OpenRocket's own discretized implementation value.
    let or_impl = f(&fx, &["totals", "openrocket_implementation_cross_check_cp_x_m"]);
    assert!(rel_err(ours, or_impl) < 0.005);
}
