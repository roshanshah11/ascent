//! Day 7 acceptance: the solver repairs a deliberately infeasible
//! configuration, deterministically, with a complete diff.

use ascent_aero::{BodyTube, FinSet, NoseCone, NoseShape, RulePack, Vehicle};
use ascent_domain::Motor;
use ascent_review::{evaluate, solve, ChuteConfig, ReviewDesign, BALLAST_NAME};

const C6_JSON: &str = include_str!("../../ascent-domain/data/motors/estes_c6.json");
const IREC_JSON: &str = include_str!("../../../data/rules/irec-2026.json");

fn c6() -> Motor {
    Motor::from_json(C6_JSON).unwrap()
}

/// Light 25 mm sport rocket on a C6 — flies to ~373 m off the 5.18 m rail.
fn base_design() -> ReviewDesign {
    ReviewDesign {
        vehicle: Vehicle {
            name: "review test".into(),
            nose: NoseCone {
                length_m: 0.10,
                base_diameter_m: 0.025,
                mass_kg: 0.006,
            },
            body: BodyTube {
                length_m: 0.25,
                outer_diameter_m: 0.025,
                mass_kg: 0.016,
            },
            fins: FinSet {
                count: 3,
                root_chord_m: 0.05,
                tip_chord_m: 0.025,
                span_m: 0.04,
                sweep_m: 0.02,
                position_from_nose_m: 0.30,
                mass_kg: 0.005,
            },
            point_masses: vec![],
            motor_position_from_nose_m: 0.28,
        },
        nose_shape: NoseShape::Ogive,
        cd: 0.60,
        chute: Some(ChuteConfig {
            cd: 0.75,
            area_m2: std::f64::consts::PI * 0.15 * 0.15,
        }),
        // IREC's published single-stage rail: 17 ft = 5.18 m
        // (rule irec-2026-published-rail-length-single-stage).
        rail_length_m: 5.18,
        motor_designation: "C6".into(),
    }
}

fn rules() -> RulePack {
    RulePack::from_irec_json(IREC_JSON).unwrap()
}

#[test]
fn evaluate_reports_every_rule_with_citation() {
    let review = evaluate(&base_design(), &c6(), &rules());
    assert_eq!(review.checks.len(), 5);
    assert!(review.checks.iter().all(|c| !c.citation.is_empty()));
    assert!(review.apogee_m > 100.0);
}

#[test]
fn solver_repairs_apogee_overshoot_with_ballast() {
    // Deliberately infeasible for the mission: the bare rocket overshoots a
    // 350 m target by >15 m. The repair must hit the target AND keep every
    // IREC band green (>~15 g of nose ballast would break the 18%%-of-length
    // launch maximum, so the window is real, not slack).
    let design = base_design();
    let motor = c6();
    let rules = rules();
    let target = 350.0;

    let bare = evaluate(&design, &motor, &rules);
    assert!(
        bare.apogee_m > target + 15.0,
        "test premise: bare design must overshoot (got {:.0} m)",
        bare.apogee_m
    );

    let repair = solve(&design, &[motor], &rules, target, 3.0).expect("repairable");
    assert!((repair.achieved_apogee_m - target).abs() <= 3.0);
    assert!(repair.review.feasible, "repair must satisfy every rule");
    // The fix must be visible in the diff — ballast was added.
    assert!(repair.diff.iter().any(|d| d.field == "nose ballast"));
    // And physically present in the repaired design.
    assert!(repair
        .design
        .vehicle
        .point_masses
        .iter()
        .any(|p| p.name == BALLAST_NAME && p.mass_kg > 0.001));
}

#[test]
fn solver_is_deterministic_across_runs() {
    let design = base_design();
    let motor = c6();
    let rules = rules();
    let a = solve(&design, &[motor.clone()], &rules, 350.0, 3.0).unwrap();
    let b = solve(&design, &[motor], &rules, 350.0, 3.0).unwrap();
    assert_eq!(
        serde_json::to_string(&a.design).unwrap(),
        serde_json::to_string(&b.design).unwrap()
    );
    assert_eq!(a.achieved_apogee_m, b.achieved_apogee_m);
}

#[test]
fn unreachable_target_fails_with_named_reason() {
    let err = solve(&base_design(), &[c6()], &rules(), 5000.0, 5.0).unwrap_err();
    assert!(err.contains("C6"), "error must name the motor: {err}");
    assert!(err.contains("below target"), "error must say why: {err}");
}

#[test]
fn solver_leaves_original_design_untouched() {
    let design = base_design();
    let before = serde_json::to_string(&design).unwrap();
    let _ = solve(&design, &[c6()], &rules(), 350.0, 3.0).unwrap();
    assert_eq!(before, serde_json::to_string(&design).unwrap());
}

#[test]
fn repair_reruns_to_identical_review() {
    // The repaired design, re-evaluated from scratch, must reproduce the
    // review the solver reported — no hidden state in the repair.
    let repair = solve(&base_design(), &[c6()], &rules(), 350.0, 3.0).unwrap();
    let fresh = evaluate(&repair.design, &c6(), &rules());
    assert_eq!(
        serde_json::to_string(&fresh).unwrap(),
        serde_json::to_string(&repair.review).unwrap()
    );
}
