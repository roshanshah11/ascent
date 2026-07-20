//! Conservative, deterministic flight-readiness structural screens.
//!
//! These are intentionally margin checks, not a certification analysis. Each
//! result carries the named source and equation used so review output remains
//! auditable and can be replaced by a higher-fidelity method later.

use serde::{Deserialize, Serialize};

pub const MIN_RAIL_EXIT_VELOCITY_MS: f64 = 25.0;
pub const MIN_FLUTTER_MULTIPLE: f64 = 1.5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinMaterial {
    pub name: String,
    /// Elastic modulus in Pa, measured or sourced for the actual fin stock.
    pub elastic_modulus_pa: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralInputs {
    pub fin_span_m: f64,
    pub fin_root_chord_m: f64,
    pub fin_thickness_m: f64,
    pub fin_material: FinMaterial,
    pub air_density_kg_m3: f64,
    pub predicted_max_velocity_ms: f64,
    pub max_motor_thrust_n: f64,
    pub airframe_thrust_rating_n: f64,
    pub rail_exit_velocity_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralCheck {
    pub check_id: String,
    pub label: String,
    pub value: f64,
    pub limit: f64,
    /// Positive means margin remains; zero is a pass boundary.
    pub margin: f64,
    pub units: String,
    pub pass: bool,
    /// Named formula source or governing launch criterion.
    pub source: String,
}

/// Fin flutter screening velocity in m/s. The geometry term is the
/// cantilever-plate form from NACA TN 4197. `0.03164` is the documented
/// unit/shape coefficient for this low-aspect-ratio, subsonic screen; this is
/// deliberately a review gate, not an aeroelastic certification calculation.
pub fn fin_flutter_velocity_ms(inputs: &StructuralInputs) -> f64 {
    let numerator = inputs.fin_material.elastic_modulus_pa * inputs.fin_thickness_m.powi(3);
    let denominator =
        inputs.air_density_kg_m3 * inputs.fin_root_chord_m * inputs.fin_span_m.powi(3);
    if numerator <= 0.0 || denominator <= 0.0 {
        return 0.0;
    }
    0.03164 * (numerator / denominator).sqrt()
}

pub fn evaluate_structural(inputs: &StructuralInputs) -> Vec<StructuralCheck> {
    let flutter = fin_flutter_velocity_ms(inputs);
    let flutter_required = MIN_FLUTTER_MULTIPLE * inputs.predicted_max_velocity_ms;
    let flutter_margin = flutter - flutter_required;
    let thrust_margin = inputs.airframe_thrust_rating_n - inputs.max_motor_thrust_n;
    let rail_margin = inputs.rail_exit_velocity_ms - MIN_RAIL_EXIT_VELOCITY_MS;
    vec![
        StructuralCheck {
            check_id: "fin_flutter_velocity".into(),
            label: "Fin flutter velocity".into(),
            value: flutter,
            limit: flutter_required,
            margin: flutter_margin,
            units: "m/s".into(),
            pass: flutter_margin >= 0.0,
            source: "NACA TN 4197, Theoretical Flutter Characteristics of Cantilevered Panels (cantilever-plate screening equation); IREC 2026 rule pack (V_flutter >= 1.5 V_max)".into(),
        },
        StructuralCheck {
            check_id: "max_thrust_load".into(),
            label: "Maximum thrust load".into(),
            value: inputs.max_motor_thrust_n,
            limit: inputs.airframe_thrust_rating_n,
            margin: thrust_margin,
            units: "N".into(),
            pass: thrust_margin >= 0.0,
            source: "NASA SP-8072, Structural Design of Sounding Rockets (axial thrust load compared with documented airframe rating)".into(),
        },
        StructuralCheck {
            check_id: "rail_departure_velocity".into(),
            label: "Rail-departure velocity".into(),
            value: inputs.rail_exit_velocity_ms,
            limit: MIN_RAIL_EXIT_VELOCITY_MS,
            margin: rail_margin,
            units: "m/s".into(),
            pass: rail_margin >= 0.0,
            source: "IREC 2026 Design, Test, & Evaluation Guide §5.3.1 (minimum 25 m/s rail-departure velocity)".into(),
        },
    ]
}
