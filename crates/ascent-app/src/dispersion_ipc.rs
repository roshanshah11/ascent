//! Monte Carlo dispersion IPC (v0.2 Step 6). Coarse command: the UI sends
//! the whole Design plus a study spec, gets the whole DispersionSummary
//! back.
//!
//! The rigid-body pitch inputs (CNα, CP, CG, inertia) are provisional: the
//! simple Design DTO carries no geometry yet, so they are derived from the
//! design's diameter at a conservative 2-caliber static margin, matching
//! the Alpha III class this editor targets. The vehicle editor unifies this
//! with the ascent-aero Barrowman path (same seam review_ipc already uses).

use ascent_sim::{
    run_dispersion, Dispersion, DispersionSummary, PlanarVehicle, SimConfig, Variation,
    WindProfile,
};
use serde::{Deserialize, Serialize};

use crate::design::{build_flight, Design};

/// The UI-facing study request: base wind plus the dispersion spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispersionRequest {
    pub seed: u64,
    pub samples: u32,
    pub vary: Vec<Variation>,
    /// Mean wind, m/s, blowing downrange (+x). Variations sit on top.
    pub base_wind_ms: f64,
}

/// Provisional planar rigid-body numbers for a Design (see module docs).
fn planar_vehicle_for(design: &Design) -> PlanarVehicle {
    let d = design.diameter_mm / 1000.0;
    let radius = d / 2.0;
    // Alpha III-class proportions: length ≈ 12 calibers, CG near mid-body.
    let length = 12.0 * d;
    let mass_kg = design.dry_mass_g / 1000.0;
    PlanarVehicle {
        cn_alpha_per_rad: 10.0,
        cg_from_nose_m: 0.55 * length,
        cp_from_nose_m: 0.55 * length + 2.0 * d,
        pitch_inertia_kgm2: mass_kg * length * length / 12.0,
        reference_area_m2: std::f64::consts::PI * radius * radius,
        launch_angle_rad: 0.0,
    }
}

/// Cap runaway sample counts before they hit the solver: the IPC boundary
/// is where the UI's inputs stop being trusted.
fn capped_samples(requested: u32) -> u32 {
    requested.min(2000)
}

pub fn run(design: &Design, request: &DispersionRequest) -> Result<DispersionSummary, String> {
    let (rocket, motor, env) = build_flight(design)?;
    let vehicle = planar_vehicle_for(design);
    let wind = if request.base_wind_ms == 0.0 {
        WindProfile::calm()
    } else {
        WindProfile::constant(request.base_wind_ms)
    };
    let spec = Dispersion {
        seed: request.seed,
        samples: capped_samples(request.samples),
        vary: request.vary.clone(),
    };
    run_dispersion(&rocket, &motor, &env, &vehicle, &wind, &SimConfig::default(), &spec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ascent_sim::VaryParam;

    fn request() -> DispersionRequest {
        DispersionRequest {
            seed: 42,
            samples: 20,
            vary: vec![
                Variation { param: VaryParam::ThrustPct, sigma: 3.0 },
                Variation { param: VaryParam::WindSpeedMs, sigma: 1.5 },
            ],
            base_wind_ms: 3.0,
        }
    }

    #[test]
    fn reference_design_dispersion_is_deterministic_and_sane() {
        let design = Design::reference();
        let a = run(&design, &request()).unwrap();
        let b = run(&design, &request()).unwrap();
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap(),
            "same seed over IPC must stay byte-identical"
        );
        assert_eq!(a.runs.len(), 20);
        // Reference apogee is ~358 m; the fleet median must stay in family.
        assert!(a.apogee_p50_m > 250.0 && a.apogee_p50_m < 450.0, "p50 {}", a.apogee_p50_m);
        assert!(a.landing_mean_m > 0.0, "3 m/s mean wind lands the fleet downwind");
    }

    #[test]
    fn invalid_design_is_rejected() {
        let mut design = Design::reference();
        design.motor_designation = "Z99".into();
        assert!(run(&design, &request()).is_err());
    }

    #[test]
    fn sample_count_is_capped() {
        assert_eq!(capped_samples(100_000), 2000);
        assert_eq!(capped_samples(2000), 2000);
        assert_eq!(capped_samples(50), 50);
    }
}
