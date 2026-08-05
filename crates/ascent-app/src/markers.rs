//! Read-only CP/CG marker derivation for the viewport overlay (v0.4 R3F
//! step). Same tree → aero view the dispersion path uses, so the marker
//! positions and the flight model can never disagree. This is a query,
//! not a command: nothing here mutates the document or touches the
//! journal.

use ascent_aero::{stability_envelope, total_cp_from_nose_m, Vehicle as AeroVehicle};
use ascent_domain::vehicle::Vehicle as TreeVehicle;
use serde::{Deserialize, Serialize};

use crate::design::{build_flight, Design};

/// Stations are meters from the nose tip, matching the tree's convention.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VehicleMarkers {
    pub cp_from_nose_m: f64,
    pub cg_ignition_from_nose_m: f64,
    pub cg_burnout_from_nose_m: f64,
    pub length_m: f64,
    pub diameter_m: f64,
    pub stability_ignition_cal: f64,
    pub stability_burnout_cal: f64,
}

pub fn vehicle_markers(tree: &TreeVehicle, design: &Design) -> Result<VehicleMarkers, String> {
    let (aero, shape) = AeroVehicle::from_tree(tree)?;
    let (_, motor, _) = build_flight(design)?;
    let (stab_ignition, stab_burnout) = stability_envelope(&aero, shape, &motor);
    Ok(VehicleMarkers {
        cp_from_nose_m: total_cp_from_nose_m(&aero, shape),
        cg_ignition_from_nose_m: aero.cg_at(&motor, 0.0),
        cg_burnout_from_nose_m: aero.cg_at(&motor, motor.burn_time()),
        length_m: aero.length_m(),
        diameter_m: aero.diameter_m(),
        stability_ignition_cal: stab_ignition,
        stability_burnout_cal: stab_burnout,
    })
}
