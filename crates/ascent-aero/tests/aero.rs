//! Day 4 tests. Every expected value here is hand-derivable (degenerate
//! shapes, closed-form arithmetic) or a physical-direction assertion.
//! The kit-accurate Alpha III fixture (docs/BARROWMAN_WORKSHEET.md +
//! data/fixtures/alpha3_stability.json, Codex task A2) adds exact-value
//! pins when it lands.

use approx::assert_relative_eq;
use ascent_aero::{
    fin_set_cn, nose_cn, stability_calibers_at, stability_envelope, total_cp_from_nose_m, BodyTube,
    Comparator, FinSet, FlightQuantities, NoseCone, NoseShape, PointMass, RulePack, Vehicle,
};
use ascent_domain::Motor;

const C6_JSON: &str = include_str!("../../ascent-domain/data/motors/estes_c6.json");

fn c6() -> Motor {
    Motor::from_json(C6_JSON).unwrap()
}

/// Generic 25 mm sport rocket used across tests (not a specific kit).
fn test_vehicle() -> Vehicle {
    Vehicle {
        name: "test".into(),
        nose: NoseCone {
            length_m: 0.10,
            base_diameter_m: 0.025,
            mass_kg: 0.008,
        },
        body: BodyTube {
            length_m: 0.25,
            outer_diameter_m: 0.025,
            mass_kg: 0.020,
        },
        fins: FinSet {
            count: 3,
            root_chord_m: 0.05,
            tip_chord_m: 0.025,
            span_m: 0.04,
            sweep_m: 0.02,
            position_from_nose_m: 0.30,
            mass_kg: 0.006,
        },
        point_masses: vec![],
        motor_position_from_nose_m: 0.28,
    }
}

// ---- mass & CG ----

#[test]
fn dry_mass_is_component_sum() {
    let mut v = test_vehicle();
    v.point_masses.push(PointMass {
        name: "payload".into(),
        mass_kg: 0.010,
        position_from_nose_m: 0.12,
    });
    assert_relative_eq!(v.dry_mass_kg(), 0.008 + 0.020 + 0.006 + 0.010);
}

#[test]
fn cg_of_two_point_masses_is_weighted_average() {
    // Make structure negligible so the point masses dominate.
    let mut v = test_vehicle();
    v.nose.mass_kg = 1e-12;
    v.body.mass_kg = 1e-12;
    v.fins.mass_kg = 1e-12;
    v.point_masses = vec![
        PointMass {
            name: "a".into(),
            mass_kg: 0.030,
            position_from_nose_m: 0.10,
        },
        PointMass {
            name: "b".into(),
            mass_kg: 0.010,
            position_from_nose_m: 0.30,
        },
    ];
    // (30·0.10 + 10·0.30)/40 = 0.15
    assert_relative_eq!(v.dry_cg_from_nose_m(), 0.15, max_relative = 1e-6);
}

#[test]
fn rectangular_fin_centroid_is_half_chord_and_triangle_is_third() {
    let mut v = test_vehicle();
    v.fins.root_chord_m = 0.06;
    v.fins.tip_chord_m = 0.06;
    v.fins.sweep_m = 0.0;
    assert_relative_eq!(v.fins.planform_centroid_m(), 0.03, max_relative = 1e-9);

    v.fins.tip_chord_m = 0.0;
    assert_relative_eq!(v.fins.planform_centroid_m(), 0.02, max_relative = 1e-9);
}

#[test]
fn cg_moves_forward_as_rear_motor_burns() {
    let v = test_vehicle();
    let motor = c6();
    let cg0 = v.cg_at(&motor, 0.0);
    let cg_mid = v.cg_at(&motor, motor.burn_time() / 2.0);
    let cg_end = v.cg_at(&motor, motor.burn_time());
    assert!(cg_mid < cg0, "CG must move toward the nose during burn");
    assert!(cg_end < cg_mid);
    // Mass bookkeeping: propellant fully gone at burnout.
    assert_relative_eq!(
        v.loaded_mass_at(&motor, motor.burn_time()),
        v.dry_mass_kg() + motor.total_mass_kg - motor.propellant_mass_kg,
        max_relative = 1e-9
    );
}

// ---- Barrowman ----

#[test]
fn conical_nose_cp_is_two_thirds_length_with_cn_alpha_two() {
    let nose = nose_cn(NoseShape::Conical, 0.12);
    assert_relative_eq!(nose.cn_alpha, 2.0);
    assert_relative_eq!(nose.cp_from_nose_m, 0.08, max_relative = 1e-9);
}

#[test]
fn rectangular_fin_cp_is_quarter_chord() {
    // Classic Barrowman result: unswept rectangular fin CP at c/4.
    let mut v = test_vehicle();
    v.fins.root_chord_m = 0.06;
    v.fins.tip_chord_m = 0.06;
    v.fins.sweep_m = 0.0;
    let fins = fin_set_cn(&v);
    assert_relative_eq!(
        fins.cp_from_nose_m - v.fins.position_from_nose_m,
        0.015,
        max_relative = 1e-9
    );
}

#[test]
fn fin_cn_alpha_matches_hand_computation() {
    // Hand-computed: 3 rectangular fins, c = 0.05, s = 0.04, d = 0.025, m = 0.
    // mid-chord offset = 0, l = s = 0.04
    // raw = 4·3·(0.04/0.025)² / (1 + √(1 + (2·0.04/0.10)²)) = 30.72 / 2.2806
    // K = 1 + 0.0125/(0.04+0.0125) = 1.238095
    let mut v = test_vehicle();
    v.fins.root_chord_m = 0.05;
    v.fins.tip_chord_m = 0.05;
    v.fins.sweep_m = 0.0;
    let raw = 4.0 * 3.0 * (0.04_f64 / 0.025).powi(2)
        / (1.0 + (1.0_f64 + (2.0_f64 * 0.04 / 0.10).powi(2)).sqrt());
    let k = 1.0 + 0.0125 / (0.04 + 0.0125);
    let fins = fin_set_cn(&v);
    assert_relative_eq!(fins.cn_alpha, k * raw, max_relative = 1e-12);
}

#[test]
fn total_cp_lies_between_nose_and_fin_cp_closer_to_fins() {
    let v = test_vehicle();
    let nose = nose_cn(NoseShape::Ogive, v.nose.length_m);
    let fins = fin_set_cn(&v);
    let total = total_cp_from_nose_m(&v, NoseShape::Ogive);
    assert!(total > nose.cp_from_nose_m && total < fins.cp_from_nose_m);
    // Fins carry more CNα than the nose on any sane sport rocket.
    assert!(fins.cn_alpha > nose.cn_alpha);
    assert!(
        (fins.cp_from_nose_m - total) < (total - nose.cp_from_nose_m),
        "total CP must sit closer to the dominant (fin) CP"
    );
}

// ---- stability: physical-direction acceptance tests ----

#[test]
fn stability_increases_through_burn() {
    let v = test_vehicle();
    let motor = c6();
    let (at_ignition, at_burnout) = stability_envelope(&v, NoseShape::Ogive, &motor);
    assert!(
        at_burnout > at_ignition,
        "rear motor burning off must increase margin"
    );
}

#[test]
fn larger_fin_span_moves_cp_aft_and_increases_stability() {
    let v = test_vehicle();
    let motor = c6();
    let mut bigger = v.clone();
    bigger.fins.span_m *= 1.5;
    assert!(
        total_cp_from_nose_m(&bigger, NoseShape::Ogive)
            > total_cp_from_nose_m(&v, NoseShape::Ogive)
    );
    assert!(
        stability_calibers_at(&bigger, NoseShape::Ogive, &motor, 0.0)
            > stability_calibers_at(&v, NoseShape::Ogive, &motor, 0.0)
    );
}

#[test]
fn nose_payload_moves_cg_forward_and_increases_stability() {
    let v = test_vehicle();
    let motor = c6();
    let mut loaded = v.clone();
    loaded.point_masses.push(PointMass {
        name: "nose payload".into(),
        mass_kg: 0.015,
        position_from_nose_m: 0.05,
    });
    assert!(loaded.dry_cg_from_nose_m() < v.dry_cg_from_nose_m());
    assert!(
        stability_calibers_at(&loaded, NoseShape::Ogive, &motor, 0.0)
            > stability_calibers_at(&v, NoseShape::Ogive, &motor, 0.0)
    );
}

#[test]
fn tail_mass_decreases_stability() {
    let v = test_vehicle();
    let motor = c6();
    let mut tail_heavy = v.clone();
    tail_heavy.point_masses.push(PointMass {
        name: "tail ballast".into(),
        mass_kg: 0.015,
        position_from_nose_m: 0.33,
    });
    assert!(
        stability_calibers_at(&tail_heavy, NoseShape::Ogive, &motor, 0.0)
            < stability_calibers_at(&v, NoseShape::Ogive, &motor, 0.0)
    );
}

// ---- constraints ----

#[test]
fn constraint_boundaries_pass_at_value_fail_below() {
    let pack = RulePack::builtin_defaults();
    let exactly_at = FlightQuantities {
        rail_exit_velocity_ms: 25.0,
        min_stability_calibers: 1.5,
        fin_span_calibers: 0.8,
        stability_pct_len_at_launch: 12.0,
        stability_pct_len_min: 9.0,
        stability_pct_len_max: 20.0,
    };
    assert!(pack.check(&exactly_at).iter().all(|c| c.pass));

    let just_below = FlightQuantities {
        rail_exit_velocity_ms: 24.999,
        min_stability_calibers: 1.499,
        fin_span_calibers: 0.799,
        stability_pct_len_at_launch: 12.0,
        stability_pct_len_min: 9.0,
        stability_pct_len_max: 20.0,
    };
    let results = pack.check(&just_below);
    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|c| !c.pass));
    // Every result must carry its citation — no floating numbers.
    assert!(results.iter().all(|c| !c.citation.is_empty()));
}

#[test]
fn rule_pack_round_trips_through_json_and_skips_unknown_quantities() {
    let json = r#"{
        "name": "irec-2026-subset",
        "rules": [
            {"id": "rail-exit", "description": "min rail exit", "quantity": "rail_exit_velocity_ms",
             "comparator": "gte", "value": 30.5, "units": "m/s", "citation": "DTEG example"},
            {"id": "future", "description": "not yet measured", "quantity": "max_mach",
             "comparator": "lte", "value": 0.9, "units": "mach", "citation": "DTEG example"}
        ]
    }"#;
    let pack = RulePack::from_json(json).unwrap();
    let q = FlightQuantities {
        rail_exit_velocity_ms: 31.0,
        min_stability_calibers: 2.0,
        fin_span_calibers: 1.0,
        stability_pct_len_at_launch: 12.0,
        stability_pct_len_min: 9.0,
        stability_pct_len_max: 20.0,
    };
    let results = pack.check(&q);
    // Unknown quantity is skipped, not falsely passed or failed.
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].rule_id, "rail-exit");
    assert!(results[0].pass);
    assert_eq!(results[0].comparator, Comparator::Gte);
}

#[test]
fn irec_2026_rule_pack_loads_with_citations() {
    // The cited pack produced by task A1 (data/rules/irec-2026.json).
    let json = include_str!("../../../data/rules/irec-2026.json");
    let pack = ascent_aero::RulePack::from_irec_json(json).unwrap();

    // All five evaluable rules extracted, none invented.
    assert_eq!(pack.rules.len(), 5);
    let rail = pack
        .rules
        .iter()
        .find(|r| r.id == "irec-2026-rail-departure-velocity-minimum")
        .expect("rail rule present");
    assert_eq!(rail.value, 25.0);
    assert_eq!(rail.comparator, Comparator::Gte);
    // Every extracted rule carries a real document citation with a section.
    for rule in &pack.rules {
        assert!(
            rule.citation.contains('§'),
            "no section in {}",
            rule.citation
        );
        assert!(
            !rule.citation.contains('?'),
            "unresolved section in {}",
            rule.citation
        );
    }

    // A vehicle inside all bands passes; stability 12% of length at launch.
    let pass_q = FlightQuantities {
        rail_exit_velocity_ms: 30.0,
        min_stability_calibers: 2.0,
        fin_span_calibers: 1.0,
        stability_pct_len_at_launch: 12.0,
        stability_pct_len_min: 9.0,
        stability_pct_len_max: 20.0,
    };
    assert!(pack.check(&pass_q).iter().all(|c| c.pass));

    // Over-stable at launch (19% > 18% max) fails exactly that rule.
    let mut over = pass_q;
    over.stability_pct_len_at_launch = 19.0;
    let results = pack.check(&over);
    let failed: Vec<_> = results.iter().filter(|c| !c.pass).collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].rule_id, "irec-2026-stability-maximum-at-launch");
}

#[test]
fn stability_pct_of_length_consistent_with_calibers() {
    use ascent_aero::stability_pct_of_length_at;
    let v = test_vehicle();
    let motor = c6();
    let cal = stability_calibers_at(&v, NoseShape::Ogive, &motor, 0.0);
    let pct = stability_pct_of_length_at(&v, NoseShape::Ogive, &motor, 0.0);
    // Same (CP − CG), two denominators: pct = cal · d / L · 100.
    assert_relative_eq!(
        pct,
        cal * v.diameter_m() / v.length_m() * 100.0,
        max_relative = 1e-12
    );
}

#[test]
fn fin_span_quantity_in_calibers() {
    let v = test_vehicle();
    // span 0.04 m on a 0.025 m body = 1.6 calibers — passes the 0.8 minimum.
    let span_cal = v.fins.span_m / v.diameter_m();
    assert_relative_eq!(span_cal, 1.6, max_relative = 1e-9);
}
