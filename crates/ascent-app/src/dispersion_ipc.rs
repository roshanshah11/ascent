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
    run_dispersion_observed, AtmosphereProfile, Dispersion, DispersionSummary, PlanarVehicle,
    SimConfig, Variation, WindProfile,
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
    atmosphere: Option<&AtmosphereProfile>,
    request: &DispersionRequest,
) -> Result<DispersionSummary, String> {
    run_observed(tree, design, atmosphere, request, |_, _| true)
        .map(|outcome| outcome.expect("uncancellable run cannot be cancelled"))
}

/// The job runner's entry: same run, with a progress observer that may
/// cancel cooperatively (`Ok(None)`). The observer never touches the RNG
/// stream, so observed and plain runs are byte-identical for a seed.
pub fn run_observed(
    tree: &TreeVehicle,
    design: &Design,
    atmosphere: Option<&AtmosphereProfile>,
    request: &DispersionRequest,
    on_progress: impl FnMut(u32, u32) -> bool,
) -> Result<Option<DispersionSummary>, String> {
    let (rocket, motor, mut env) = build_flight(design)?;
    let vehicle = planar_vehicle_from_tree(tree)?;
    // An imported atmosphere (v0.5) overrides the request's synthetic
    // constant wind, and its measured densities (when present) override
    // the analytic density model. It is part of the study input hash, so
    // this substitution is provenance-tracked, not silent.
    let wind = match atmosphere {
        Some(profile) => {
            if let Some(model) = profile.atmosphere_model() {
                env.atmosphere = model;
            }
            profile.wind_profile()
        }
        None if request.base_wind_ms == 0.0 => WindProfile::calm(),
        None => WindProfile::constant(request.base_wind_ms),
    };
    let spec = Dispersion {
        seed: request.seed,
        samples: capped_samples(request.samples),
        vary: request.vary.clone(),
    };
    run_dispersion_observed(
        &rocket,
        &motor,
        &env,
        &vehicle,
        &wind,
        &SimConfig::default(),
        &spec,
        on_progress,
    )
}
