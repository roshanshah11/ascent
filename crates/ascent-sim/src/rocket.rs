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

/// Drogue chute for dual-deploy recovery: out at apogee, rides to the
/// ground alongside the main once it opens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Drogue {
    pub cd: f64,
    pub area_m2: f64,
}

/// Parachute recovery. Single-deploy (the default): the main opens at
/// apogee. Dual-deploy: set `main_deploy_altitude_m` — the drogue (if
/// any) opens at apogee and the main opens descending through that
/// altitude. Both new fields are serde-defaulted and skipped when absent
/// so pre-v0.5 documents, journals, and study input hashes are untouched.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recovery {
    pub chute_cd: f64,
    pub chute_area_m2: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drogue: Option<Drogue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_deploy_altitude_m: Option<f64>,
}

impl Recovery {
    /// Convenience for the pervasive single-deploy case (and every
    /// pre-v0.5 call site).
    pub fn single(chute_cd: f64, chute_area_m2: f64) -> Self {
        Self {
            chute_cd,
            chute_area_m2,
            drogue: None,
            main_deploy_altitude_m: None,
        }
    }

    /// True when a main-deploy altitude is configured (dual-deploy).
    pub fn is_dual_deploy(&self) -> bool {
        self.main_deploy_altitude_m.is_some()
    }

    /// Total chute CdA in effect at `altitude_m` during descent. With no
    /// main-deploy altitude this is the main's CdA everywhere — the exact
    /// pre-v0.5 expression, so single-deploy flights are byte-identical.
    /// Dual-deploy: drogue CdA (possibly zero) above the deploy altitude,
    /// drogue + main at or below it.
    pub fn descent_cda(&self, altitude_m: f64) -> f64 {
        let main_cda = self.chute_cd * self.chute_area_m2;
        match self.main_deploy_altitude_m {
            None => main_cda,
            Some(deploy_m) => {
                let drogue_cda = self
                    .drogue
                    .as_ref()
                    .map(|d| d.cd * d.area_m2)
                    .unwrap_or(0.0);
                if altitude_m <= deploy_m {
                    drogue_cda + main_cda
                } else {
                    drogue_cda
                }
            }
        }
    }
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
