use serde::{Deserialize, Serialize};

use crate::atmosphere::AtmosphereModel;

/// Quadratic drag model: D = 0.5 * rho * cd * area * v².
///
/// Day-2 assumption (declared): constant Cd. Mach-dependent Cd arrives with
/// the Day 3–4 aero work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DragModel {
    pub cd: f64,
    pub reference_area_m2: f64,
}

/// Parachute recovery, deployed at apogee (motor-eject delay modeling
/// arrives with the Day 4 vehicle model).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recovery {
    pub chute_cd: f64,
    pub chute_area_m2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rocket {
    pub name: String,
    /// Structure mass without the motor, kg.
    pub dry_mass_kg: f64,
    /// None = drag-free (used by analytic fixtures).
    pub drag: Option<DragModel>,
    /// None = ballistic to the ground after apogee.
    #[serde(default)]
    pub recovery: Option<Recovery>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub gravity_ms2: f64,
    pub atmosphere: AtmosphereModel,
    /// Launch rail/rod length, metres. Rail exit is reported as an event
    /// and (from Day 4) checked against the IREC 25 m/s minimum.
    pub rail_length_m: f64,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            gravity_ms2: 9.80665,
            atmosphere: AtmosphereModel::Standard,
            rail_length_m: 0.9,
        }
    }
}
