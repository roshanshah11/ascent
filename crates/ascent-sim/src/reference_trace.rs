//! Deterministic two-stage 6-DOF trace for an evidence-graded reference
//! mission.
//!
//! This lives in `ascent-sim` (not `ascent-domain`) because the trace consumes
//! `simulate_sixdof_staged`, and `ascent-domain` must not depend on
//! `ascent-sim`. The reference *definition* (geometry, sources, evidence) lives
//! in `ascent_domain::reference`; this module maps that definition to a staged
//! 6-DOF run.

use ascent_domain::reference::ReferenceMission;
use ascent_domain::vehicle::{Part, PartKind, Vehicle};

use crate::rocket::DragModel;
use crate::run_session::MissionSnapshot;
use crate::sixdof::{simulate_sixdof_staged, SixDofResult, SixDofStage, SixDofVehicle};

/// Map a [`Vehicle`]'s part tree into a sequence of [`SixDofStage`]s, one per
/// detected stage, using the bundled motor registry to resolve motor
/// designations. The mapping is deterministic: identical inputs always yield
/// identical stages and therefore an identical trace identity.
pub fn mission_to_sixdof_stages(mission: &ReferenceMission) -> Result<Vec<SixDofStage>, String> {
    let vehicle = &mission.vehicle;
    let stage_infos = vehicle.stages();
    if stage_infos.is_empty() {
        return Err("vehicle has no detectable stages".into());
    }
    let mut stages = Vec::with_capacity(stage_infos.len());
    // Domain stages are nose-first; the simulator requires burn order.
    for info in stage_infos.iter().rev() {
        let designation = info
            .motor_designation
            .as_ref()
            .ok_or_else(|| "stage without a motor designation cannot be traced".to_string())?;
        let motor = mission
            .motors
            .iter()
            .find(|motor| motor.designation == *designation)
            .ok_or_else(|| format!("motor {designation} missing from reference mission"))?
            .clone();
        let sv = stage_sixdof_vehicle(vehicle, info.part_range);
        let reference_area_m2 = sv.reference_area_m2;
        stages.push(SixDofStage {
            dry_mass_kg: info.dry_mass_g / 1000.0,
            motor,
            separation_delay_s: info.separation_delay_s.unwrap_or(0.0),
            vehicle: sv,
            drag: Some(DragModel {
                cd: 0.45,
                reference_area_m2,
            }),
        });
    }
    Ok(stages)
}

/// Geometry/loading accumulator used while walking one stage's parts.
struct GeomItem {
    mass_kg: f64,
    /// Station (m) of the point mass from the stage nose.
    station_m: f64,
    /// Own (spin) inertia about the part's own CG, kg·m².
    own_inertia_kgm2: f64,
}

/// Geometric length of a part along the airframe axis (0 for attachments).
fn part_structural_length(part: &Part) -> f64 {
    match &part.kind {
        PartKind::NoseCone { length_m, .. }
        | PartKind::BodyTube { length_m, .. }
        | PartKind::Transition { length_m, .. }
        | PartKind::StageCoupler { length_m, .. } => *length_m,
        _ => 0.0,
    }
}

/// Derive a [`SixDofVehicle`] for a single stage from its part segment.
///
/// Stations accumulate nose-first. CG / inertia follow the same convention as
/// `Vehicle::mass_properties` (nose cones at 2/3 length, tubes/couplers at
/// mid-span, attachments at their parent's aft). Reference aerodynamic
/// coefficients are constant reference values — the Black Brant IX mission
/// grades `vehicle.geometry`/`vehicle.mass` as *approximate*, so this mapping
/// is intentionally a reference approximation, not a fidelity claim.
fn stage_sixdof_vehicle(vehicle: &Vehicle, range: (usize, usize)) -> SixDofVehicle {
    let segment = &vehicle.parts[range.0..range.1];
    let mut cursor = 0.0_f64;
    let mut items: Vec<GeomItem> = Vec::new();
    let mut r_max = 0.0_f64;
    for part in segment {
        let fore = cursor;
        let len = part_structural_length(part);
        walk_part(part, fore, len, &mut items, &mut r_max);
        cursor += len;
    }
    let total_kg: f64 = items.iter().map(|i| i.mass_kg).sum();
    let cg = if total_kg > 0.0 {
        items.iter().map(|i| i.mass_kg * i.station_m).sum::<f64>() / total_kg
    } else {
        0.0
    };
    let inertia = items
        .iter()
        .map(|i| i.own_inertia_kgm2 + i.mass_kg * (i.station_m - cg).powi(2))
        .sum::<f64>();
    let reference_area_m2 = std::f64::consts::PI * r_max * r_max;
    SixDofVehicle {
        cg_from_nose_m: cg,
        pitch_yaw_inertia_kgm2: inertia,
        cp_from_nose_m: (0.65 * cursor).max(0.0),
        cn_alpha_per_rad: 8.0,
        reference_area_m2,
    }
}

/// Recurse a part subtree, pushing point-mass items and tracking the max body
/// radius. `fore`/`len` are the parent's fore station and structural length.
fn walk_part(part: &Part, fore: f64, len: f64, items: &mut Vec<GeomItem>, r_max: &mut f64) {
    match &part.kind {
        PartKind::NoseCone {
            length_m,
            base_radius_m,
            mass_g,
            ..
        } => {
            *r_max = r_max.max(*base_radius_m);
            items.push(GeomItem {
                mass_kg: *mass_g / 1000.0,
                station_m: fore + (2.0 / 3.0) * length_m,
                own_inertia_kgm2: (*mass_g / 1000.0) * length_m.powi(2) / 12.0,
            });
        }
        PartKind::BodyTube {
            length_m,
            outer_radius_m,
            mass_g,
            ..
        }
        | PartKind::StageCoupler {
            length_m,
            outer_radius_m,
            mass_g,
            ..
        } => {
            *r_max = r_max.max(*outer_radius_m);
            items.push(GeomItem {
                mass_kg: *mass_g / 1000.0,
                station_m: fore + length_m / 2.0,
                own_inertia_kgm2: (*mass_g / 1000.0) * length_m.powi(2) / 12.0,
            });
        }
        PartKind::Transition {
            length_m,
            fore_radius_m,
            aft_radius_m,
            mass_g,
        } => {
            *r_max = r_max.max(fore_radius_m.max(*aft_radius_m));
            items.push(GeomItem {
                mass_kg: *mass_g / 1000.0,
                station_m: fore + length_m / 2.0,
                own_inertia_kgm2: (*mass_g / 1000.0) * length_m.powi(2) / 12.0,
            });
        }
        PartKind::FinSet {
            root_chord_m,
            mass_g,
            ..
        } => {
            items.push(GeomItem {
                mass_kg: *mass_g / 1000.0,
                station_m: fore + len - root_chord_m / 2.0,
                own_inertia_kgm2: 0.0,
            });
        }
        PartKind::MotorMount {
            length_m,
            position_m,
            mass_g,
            ..
        } => {
            items.push(GeomItem {
                mass_kg: *mass_g / 1000.0,
                station_m: fore + position_m + length_m / 2.0,
                own_inertia_kgm2: (*mass_g / 1000.0) * length_m.powi(2) / 12.0,
            });
        }
        PartKind::MassComponent {
            position_m, mass_g, ..
        }
        | PartKind::Parachute {
            position_m, mass_g, ..
        } => {
            items.push(GeomItem {
                mass_kg: *mass_g / 1000.0,
                station_m: fore + position_m,
                own_inertia_kgm2: 0.0,
            });
        }
    }
    for child in &part.children {
        walk_part(child, fore, len, items, r_max);
    }
}

/// Run the deterministic two-stage 6-DOF trace for a reference mission.
///
/// The trace is a pure function of the mission's vehicle + evidence-bound motors:
/// identical inputs always produce a [`SixDofResult`] with an identical
/// `summary.input_hash` (the canonical-input SHA-256).
///
/// The fixed reference inputs (calm wind, vertical launch, default
/// environment, the dt/max-time integration config, and the recovery canopy)
/// live in exactly one place: [`MissionSnapshot::from_reference_mission`]. This
/// batch path and the stepped [`crate::run_session::RunSession`] therefore run
/// byte-identical inputs by construction. The flight reaches ~97 km apogee and
/// descends under the canopy to a landing near t≈3900 s, well inside the cap.
pub fn trace_reference_mission(mission: &ReferenceMission) -> Result<SixDofResult, String> {
    let snapshot = MissionSnapshot::from_reference_mission(mission)?;
    simulate_sixdof_staged(
        &snapshot.stages,
        snapshot.recovery.clone(),
        &snapshot.wind,
        &snapshot.launch,
        &snapshot.environment,
        &snapshot.config,
    )
}
