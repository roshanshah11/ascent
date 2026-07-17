use serde::{Deserialize, Serialize};

/// Quadratic drag model: D = 0.5 * rho * cd * area * v^2.
///
/// Day-1 assumption (declared): constant Cd, constant sea-level density.
/// Mach-dependent Cd and a standard atmosphere arrive on Day 2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DragModel {
    pub cd: f64,
    pub reference_area_m2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rocket {
    pub name: String,
    /// Structure mass without the motor, kg.
    pub dry_mass_kg: f64,
    /// None = drag-free (used by analytic fixtures).
    pub drag: Option<DragModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub gravity_ms2: f64,
    pub air_density_kgm3: f64,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            gravity_ms2: 9.80665,
            air_density_kgm3: 1.225,
        }
    }
}
