//! Tree → staged-flight derivation (v0.4 multi-stage). Turns a vehicle
//! tree with `StageCoupler` parts into the per-phase inputs the staged
//! engines take: for each burn phase, the attached sub-stack's Barrowman
//! CN/CP (composed per fin set — the single-fin-set aero view can't hold
//! a booster and sustainer set at once), dry CG/inertia from the mass
//! rollup, and the motor resolved from the phase's active stage.
//!
//! Same one-source-of-truth rule as the single-stage path: the tree that
//! renders is the tree that flies.

use ascent_aero::{fin_set_cn, nose_cn, Vehicle as AeroVehicle};
use ascent_domain::vehicle::{Part, PartKind, Vehicle as TreeVehicle};
use ascent_sim::{DragModel, PlanarStage, PlanarVehicle, SixDofStage, SixDofVehicle};

use crate::design::find_motor;

/// Numbers describing one burn phase's attached stack.
struct PhaseParams {
    cn_alpha_per_rad: f64,
    cp_from_nose_m: f64,
    cg_from_nose_m: f64,
    inertia_kgm2: f64,
    reference_area_m2: f64,
}

/// Remove every fin set from the tree except the `keep`-th one
/// (0-indexed in root-part order), so `from_tree` can score fin sets one
/// at a time.
fn keep_one_fin_set(parts: &[Part], keep: usize) -> Vec<Part> {
    let mut seen = 0usize;
    parts
        .iter()
        .map(|part| {
            let mut part = part.clone();
            part.children.retain(|child| {
                if matches!(child.kind, PartKind::FinSet { .. }) {
                    let keep_this = seen == keep;
                    seen += 1;
                    keep_this
                } else {
                    true
                }
            });
            part
        })
        .collect()
}

fn count_fin_sets(parts: &[Part]) -> usize {
    parts
        .iter()
        .flat_map(|p| &p.children)
        .filter(|c| matches!(c.kind, PartKind::FinSet { .. }))
        .count()
}

/// Barrowman CN/CP for a sub-stack that may carry several fin sets:
/// nose + each fin set scored in isolation, then CNα-weighted.
fn phase_params(sub: &TreeVehicle) -> Result<PhaseParams, String> {
    let fin_sets = count_fin_sets(&sub.parts);
    if fin_sets == 0 {
        return Err("stage stack needs at least one fin set".into());
    }
    let mut components = Vec::new();
    let mut reference_area = 0.0;
    for keep in 0..fin_sets {
        let reduced = TreeVehicle {
            name: sub.name.clone(),
            parts: keep_one_fin_set(&sub.parts, keep),
        };
        let (aero, shape) = AeroVehicle::from_tree(&reduced)?;
        if keep == 0 {
            components.push(nose_cn(shape, aero.nose.length_m));
            let d = aero.diameter_m();
            reference_area = std::f64::consts::PI * d * d / 4.0;
        }
        components.push(fin_set_cn(&aero));
    }
    let total_cn: f64 = components.iter().map(|c| c.cn_alpha).sum();
    let cp = components
        .iter()
        .map(|c| c.cn_alpha * c.cp_from_nose_m)
        .sum::<f64>()
        / total_cn;
    let props = sub.mass_properties();
    Ok(PhaseParams {
        cn_alpha_per_rad: total_cn,
        cp_from_nose_m: cp,
        cg_from_nose_m: props.cg_from_nose_m,
        inertia_kgm2: props.longitudinal_moi_kg_m2,
        reference_area_m2: reference_area,
    })
}

/// Shared derivation: burn-order list of (params, dry mass to drop,
/// motor, separation delay).
fn burn_phases(
    tree: &TreeVehicle,
) -> Result<Vec<(PhaseParams, f64, ascent_domain::Motor, f64)>, String> {
    tree.validate()?;
    let stages = tree.stages();
    let mut phases = Vec::new();
    // Burn order: bottom (last nose-first stage) first.
    for (k, stage) in stages.iter().enumerate().rev() {
        let sub = TreeVehicle {
            name: tree.name.clone(),
            parts: tree.parts[0..stage.part_range.1].to_vec(),
        };
        let params = phase_params(&sub)?;
        let designation = stage
            .motor_designation
            .as_deref()
            .ok_or_else(|| format!("stage {k} has no motor mount"))?;
        let motor = find_motor(designation)?;
        let delay = stage.separation_delay_s.unwrap_or(0.0);
        phases.push((params, stage.dry_mass_g / 1000.0, motor, delay));
    }
    Ok(phases)
}

pub fn planar_stages_from_tree(tree: &TreeVehicle, cd: f64) -> Result<Vec<PlanarStage>, String> {
    Ok(burn_phases(tree)?
        .into_iter()
        .map(|(p, dry_kg, motor, delay)| PlanarStage {
            dry_mass_kg: dry_kg,
            motor,
            separation_delay_s: delay,
            vehicle: PlanarVehicle {
                cn_alpha_per_rad: p.cn_alpha_per_rad,
                cp_from_nose_m: p.cp_from_nose_m,
                cg_from_nose_m: p.cg_from_nose_m,
                pitch_inertia_kgm2: p.inertia_kgm2,
                reference_area_m2: p.reference_area_m2,
                launch_angle_rad: 0.0,
            },
            drag: Some(DragModel {
                cd,
                reference_area_m2: p.reference_area_m2,
            }),
        })
        .collect())
}

pub fn sixdof_stages_from_tree(tree: &TreeVehicle, cd: f64) -> Result<Vec<SixDofStage>, String> {
    Ok(burn_phases(tree)?
        .into_iter()
        .map(|(p, dry_kg, motor, delay)| SixDofStage {
            dry_mass_kg: dry_kg,
            motor,
            separation_delay_s: delay,
            vehicle: SixDofVehicle {
                cg_from_nose_m: p.cg_from_nose_m,
                pitch_yaw_inertia_kgm2: p.inertia_kgm2,
                cp_from_nose_m: p.cp_from_nose_m,
                cn_alpha_per_rad: p.cn_alpha_per_rad,
                reference_area_m2: p.reference_area_m2,
            },
            drag: Some(DragModel {
                cd,
                reference_area_m2: p.reference_area_m2,
            }),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ascent_domain::vehicle::{reference_vehicle, two_stage_reference_vehicle};
    use ascent_sim::{
        simulate_planar_staged, simulate_sixdof_staged, Environment, Recovery, SimConfig,
        SixDofLaunch, Wind3DProfile, WindProfile,
    };

    fn recovery() -> Recovery {
        Recovery {
            chute_cd: 0.75,
            chute_area_m2: std::f64::consts::PI * 0.15 * 0.15,
            drogue: None,
            main_deploy_altitude_m: None,
        }
    }

    fn config() -> SimConfig {
        SimConfig {
            dt_s: 0.001,
            max_time_s: 240.0,
        }
    }

    #[test]
    fn two_stage_tree_yields_two_burn_phases_in_burn_order() {
        let stages = planar_stages_from_tree(&two_stage_reference_vehicle(), 0.6).unwrap();
        assert_eq!(stages.len(), 2);
        // Booster (D12, full stack) burns first.
        assert_eq!(stages[0].motor.designation, "D12");
        assert_eq!(stages[1].motor.designation, "C6");
        // Full stack is heavier, longer, and its CP sits aft of the
        // sustainer-alone CP (booster fins pull it aft).
        assert!(stages[0].dry_mass_kg < stages[1].dry_mass_kg + 0.03); // booster drops 27 g
        assert!((stages[0].dry_mass_kg - 0.027).abs() < 1e-9);
        assert!((stages[1].dry_mass_kg - 0.034).abs() < 1e-9);
        assert!(stages[0].vehicle.cp_from_nose_m > stages[1].vehicle.cp_from_nose_m);
        assert!(stages[0].vehicle.cg_from_nose_m > stages[1].vehicle.cg_from_nose_m);
        assert!(stages[0].vehicle.pitch_inertia_kgm2 > stages[1].vehicle.pitch_inertia_kgm2);
    }

    #[test]
    fn single_stage_tree_matches_the_single_fin_set_path() {
        let stages = planar_stages_from_tree(&reference_vehicle(), 0.6).unwrap();
        assert_eq!(stages.len(), 1);
        let (aero, shape) = ascent_aero::Vehicle::from_tree(&reference_vehicle()).unwrap();
        let expected_cp = ascent_aero::total_cp_from_nose_m(&aero, shape);
        assert!((stages[0].vehicle.cp_from_nose_m - expected_cp).abs() < 1e-12);
    }

    /// The acceptance test: the two-stage reference vehicle flies end to
    /// end in both engines with separation in the event timeline.
    #[test]
    fn two_stage_reference_flies_in_both_engines_with_separation() {
        let tree = two_stage_reference_vehicle();

        let planar = simulate_planar_staged(
            &planar_stages_from_tree(&tree, 0.6).unwrap(),
            Some(recovery()),
            &Environment::default(),
            &WindProfile::calm(),
            &config(),
        );
        let kinds: Vec<&str> = planar.events.iter().map(|e| e.kind.as_str()).collect();
        assert!(kinds.contains(&"StageSeparation"), "planar: {kinds:?}");
        assert!(kinds.contains(&"StageIgnition"), "planar: {kinds:?}");
        assert!(planar.summary.apogee_m > 0.0);
        assert!(planar.summary.landing_time_s > planar.summary.apogee_time_s);

        let sixdof = simulate_sixdof_staged(
            &sixdof_stages_from_tree(&tree, 0.6).unwrap(),
            Some(recovery()),
            &Wind3DProfile::calm(),
            &SixDofLaunch::vertical(),
            &Environment::default(),
            &config(),
        )
        .unwrap();
        let kinds: Vec<String> = sixdof
            .summary
            .events
            .iter()
            .map(|e| e.kind.clone())
            .collect();
        assert!(
            kinds.iter().any(|k| k == "StageSeparation"),
            "sixdof: {kinds:?}"
        );
        assert!(sixdof.summary.apogee_m > 0.0);

        // Cross-engine: same tree, apogees within 5%.
        let rel =
            (sixdof.summary.apogee_m - planar.summary.apogee_m).abs() / planar.summary.apogee_m;
        assert!(
            rel < 0.05,
            "{} vs {}",
            sixdof.summary.apogee_m,
            planar.summary.apogee_m
        );
    }
}
