//! The cross-validation seam (v0.2 Step 4): every solver — native RK4 today,
//! RocketPy/OpenRocket bridges later — sits behind `SimEngine`, so "run the
//! spread across engines" is a loop over `engines()`, and evidence reports
//! name exactly which engine produced a summary.

use crate::rocket::{Environment, Rocket};
use crate::sim::{simulate_vertical, SimConfig};
use crate::summary::SimSummary;
use ascent_domain::Motor;

pub trait SimEngine {
    /// Stable machine-readable identifier, e.g. "ascent-native".
    fn id(&self) -> &'static str;
    /// Engine version, surfaced verbatim in evidence reports.
    fn version(&self) -> &'static str;
    fn run(
        &self,
        rocket: &Rocket,
        motor: &Motor,
        env: &Environment,
        config: &SimConfig,
    ) -> Result<SimSummary, String>;
}

/// Ascent's built-in fixed-step RK4 vertical-flight solver behind the seam.
pub struct NativeEngine;

impl SimEngine for NativeEngine {
    fn id(&self) -> &'static str {
        "ascent-native"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn run(
        &self,
        rocket: &Rocket,
        motor: &Motor,
        env: &Environment,
        config: &SimConfig,
    ) -> Result<SimSummary, String> {
        let result = simulate_vertical(rocket, motor, env, config);
        Ok(SimSummary::from_result(&result, rocket, motor, env, config))
    }
}

/// Every engine this build knows about. Bridge engines (RocketPy, OpenRocket)
/// append here in later steps; callers select by `id()`.
pub fn engines() -> Vec<Box<dyn SimEngine>> {
    #[cfg(feature = "bridge-rocketpy")]
    {
        let mut engines: Vec<Box<dyn SimEngine>> = vec![Box::new(NativeEngine)];
        engines.push(Box::new(
            crate::rocketpy_bridge::RocketPyEngine::from_environment(),
        ));
        engines
    }
    #[cfg(not(feature = "bridge-rocketpy"))]
    {
        vec![Box::new(NativeEngine)]
    }
}
