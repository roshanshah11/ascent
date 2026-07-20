//! Flight Review IPC (Day 7). Coarse commands over the parametric review
//! rocket. v0.1 keeps the review vehicle separate from the simple design
//! editor; unifying them is part of the post-demo redesign.

use ascent_aero::{BodyTube, FinSet, NoseCone, NoseShape, RulePack, Vehicle};
use ascent_domain::{
    vehicle::{PartKind, Vehicle as TreeVehicle},
    Motor,
};
use ascent_review::{evaluate, solve, ChuteConfig, Repair, Review, ReviewDesign, StructuralConfig};
use serde::{Deserialize, Serialize};

const IREC_JSON: &str = include_str!("../../../data/rules/irec-2026.json");

fn rules() -> Result<RulePack, String> {
    RulePack::from_irec_json(IREC_JSON).map_err(|e| e.to_string())
}

fn motors() -> Vec<Motor> {
    crate::design::session_motors()
}

/// Reference vehicle for exercising the requirements-verification pipeline.
/// Its 5.18 m rail is sourced from the bundled optional IREC profile.
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
        fin_thickness_m: 0.003,
        structural: StructuralConfig::default(),
    }
}

/// Build the Review input from the same model tree that studies, evidence,
/// and the viewport consume. The reference constructor remains only for the
/// old standalone demo/test entry point.
pub fn review_from_tree(
    tree: &TreeVehicle,
    design: &crate::design::Design,
) -> Result<ReviewDesign, String> {
    let (vehicle, nose_shape) = ascent_aero::Vehicle::from_tree(tree)?;
    let fin_thickness_m = tree
        .parts
        .iter()
        .flat_map(|part| part.children.iter())
        .find_map(|part| match &part.kind {
            PartKind::FinSet { thickness_mm, .. } if *thickness_mm > 0.0 => {
                Some(*thickness_mm / 1000.0)
            }
            _ => None,
        })
        .ok_or("vehicle needs a fin set with positive thickness")?;
    let chute = design.chute.enabled.then(|| {
        let radius_m = design.chute.diameter_cm / 200.0;
        ChuteConfig {
            cd: design.chute.cd,
            area_m2: std::f64::consts::PI * radius_m * radius_m,
        }
    });
    Ok(ReviewDesign {
        vehicle,
        nose_shape,
        cd: design.cd,
        chute,
        rail_length_m: design.rail_length_m,
        motor_designation: design.motor_designation.clone(),
        fin_thickness_m,
        structural: StructuralConfig::default(),
    })
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
        review,
        target_apogee_m,
        target_tolerance_m: tol,
        target_met,
        mission_feasible,
    })
}

pub fn report_for(
    tree: &TreeVehicle,
    flat_design: &crate::design::Design,
    target_apogee_m: f64,
) -> Result<ReviewReport, String> {
    let design = review_from_tree(tree, flat_design)?;
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

pub fn repair_for(
    tree: &TreeVehicle,
    flat_design: &crate::design::Design,
    target_apogee_m: f64,
) -> Result<Repair, String> {
    let design = review_from_tree(tree, flat_design)?;
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

    #[test]
    fn tree_as_built_mass_changes_live_review_stability() {
        let mut tree = ascent_domain::vehicle::reference_vehicle();
        let flat = crate::design::Design::reference();
        let before = report_for(&tree, &flat, 350.0).unwrap();
        tree.parts[1].children[0].as_built_mass_g = Some(12.0);
        let after = report_for(&tree, &flat, 350.0).unwrap();
        assert!(
            after.design.vehicle.dry_cg_from_nose_m() > before.design.vehicle.dry_cg_from_nose_m()
        );
        assert!(
            after.review.quantities.min_stability_calibers
                < before.review.quantities.min_stability_calibers
        );
    }
}
