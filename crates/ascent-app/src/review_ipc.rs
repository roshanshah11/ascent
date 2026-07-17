//! Flight Review IPC (Day 7). Coarse commands over the parametric review
//! rocket. v0.1 keeps the review vehicle separate from the simple design
//! editor; unifying them is part of the post-demo redesign.

use ascent_aero::{BodyTube, FinSet, NoseCone, NoseShape, RulePack, Vehicle};
use ascent_domain::Motor;
use ascent_review::{evaluate, solve, ChuteConfig, Repair, Review, ReviewDesign};
use serde::{Deserialize, Serialize};

const IREC_JSON: &str = include_str!("../../../data/rules/irec-2026.json");

fn rules() -> Result<RulePack, String> {
    RulePack::from_irec_json(IREC_JSON).map_err(|e| e.to_string())
}

fn motors() -> Vec<Motor> {
    crate::design::MOTOR_SOURCES
        .iter()
        .filter_map(|s| Motor::from_json(s).ok())
        .collect()
}

/// The demo review vehicle: light 25 mm sport rocket flown from IREC's
/// published 17 ft (5.18 m) single-stage rail.
pub fn review_reference() -> ReviewDesign {
    ReviewDesign {
        vehicle: Vehicle {
            name: "Ascent review rocket".into(),
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
        rail_length_m: 5.18,
        motor_designation: "C6".into(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewReport {
    pub design: ReviewDesign,
    pub review: Review,
    pub target_apogee_m: f64,
    pub target_tolerance_m: f64,
    pub target_met: bool,
    pub mission_feasible: bool,
}

pub fn report(target_apogee_m: f64) -> Result<ReviewReport, String> {
    let design = review_reference();
    let rules = rules()?;
    let motor = motors()
        .into_iter()
        .find(|m| m.designation == design.motor_designation)
        .ok_or("reference motor missing")?;
    let review = evaluate(&design, &motor, &rules);
    let tol = 3.0;
    let target_met = (review.apogee_m - target_apogee_m).abs() <= tol;
    let mission_feasible = review.feasible && target_met;
    Ok(ReviewReport {
        design,
        target_apogee_m,
        target_tolerance_m: tol,
        target_met,
        mission_feasible,
        review,
    })
}

pub fn repair(target_apogee_m: f64) -> Result<Repair, String> {
    let design = review_reference();
    let rules = rules()?;
    solve(&design, &motors(), &rules, target_apogee_m, 3.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_misses_350_target_and_repair_fixes_it() {
        let before = report(350.0).unwrap();
        assert!(before.review.feasible, "rules pass bare");
        assert!(!before.target_met, "demo premise: target missed bare");
        assert!(!before.mission_feasible);

        let fixed = repair(350.0).unwrap();
        assert!((fixed.achieved_apogee_m - 350.0).abs() <= 3.0);
        assert!(fixed.review.feasible);
        assert!(!fixed.diff.is_empty(), "diff must show what changed");
    }
}
