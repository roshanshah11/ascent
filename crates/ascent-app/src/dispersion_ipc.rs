//! Monte Carlo dispersion IPC (v0.2 Step 6). Coarse command: the UI sends
//! the whole Design plus a study spec, gets the whole DispersionSummary
//! back.
//!
//! The rigid-body pitch inputs (CNα, CP, CG, inertia) derive from the
//! vehicle model tree (v0.3 Step 3): Barrowman CNα/CP via the ascent-aero
//! view of the tree, CG and pitch inertia from the tree's mass rollup.
//! One rocket, one source of truth — the provisional 2-caliber-margin
//! constants are gone.

use ascent_aero::{fin_set_cn, nose_cn, total_cp_from_nose_m, Vehicle as AeroVehicle};
use ascent_domain::vehicle::Vehicle as TreeVehicle;
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

/// Planar rigid-body numbers derived from the vehicle tree: Barrowman
/// CNα and CP from the aero view, CG and pitch inertia from the mass
/// rollup. The same tree drives the mesh, so the flight model and the
/// rendered rocket can never disagree.
fn planar_vehicle_from_tree(tree: &TreeVehicle) -> Result<PlanarVehicle, String> {
    let (aero, shape) = AeroVehicle::from_tree(tree)?;
    let nose = nose_cn(shape, aero.nose.length_m);
    let fins = fin_set_cn(&aero);
    let props = tree.mass_properties();
    let radius = aero.diameter_m() / 2.0;
    Ok(PlanarVehicle {
        cn_alpha_per_rad: nose.cn_alpha + fins.cn_alpha,
        cg_from_nose_m: props.cg_from_nose_m,
        cp_from_nose_m: total_cp_from_nose_m(&aero, shape),
        pitch_inertia_kgm2: props.longitudinal_moi_kg_m2,
        reference_area_m2: std::f64::consts::PI * radius * radius,
        launch_angle_rad: 0.0,
    })
}

/// Cap runaway sample counts before they hit the solver: the IPC boundary
/// is where the UI's inputs stop being trusted.
fn capped_samples(requested: u32) -> u32 {
    requested.min(2000)
}

pub fn run(
    tree: &TreeVehicle,
    design: &Design,
    request: &DispersionRequest,
) -> Result<DispersionSummary, String> {
    let (rocket, motor, env) = build_flight(design)?;
    let vehicle = planar_vehicle_from_tree(tree)?;
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
        let a = run(&ascent_domain::vehicle::reference_vehicle(), &design, &request()).unwrap();
        let b = run(&ascent_domain::vehicle::reference_vehicle(), &design, &request()).unwrap();
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
        assert!(run(&ascent_domain::vehicle::reference_vehicle(), &design, &request()).is_err());
    }

    #[test]
    fn sample_count_is_capped() {
        assert_eq!(capped_samples(100_000), 2000);
        assert_eq!(capped_samples(2000), 2000);
        assert_eq!(capped_samples(50), 50);
    }
}
