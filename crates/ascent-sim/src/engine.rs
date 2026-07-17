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
    vec![Box::new(NativeEngine)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rocket::{DragModel, Recovery};
    use crate::AtmosphereModel;

    fn reference_flight() -> (Rocket, Motor, Environment) {
        let motor = Motor::from_json(include_str!("../../ascent-domain/data/motors/estes_c6.json"))
            .expect("bundled C6 must parse");
        let rocket = Rocket {
            name: "Estes Alpha III".into(),
            dry_mass_kg: 0.034,
            drag: Some(DragModel {
                cd: 0.60,
                reference_area_m2: std::f64::consts::PI * 0.0125 * 0.0125,
            }),
            recovery: Some(Recovery {
                chute_cd: 0.75,
                chute_area_m2: std::f64::consts::PI * 0.15 * 0.15,
            }),
        };
        let env = Environment {
            gravity_ms2: 9.80665,
            atmosphere: AtmosphereModel::Standard,
            rail_length_m: 0.9,
        };
        (rocket, motor, env)
    }

    #[test]
    fn native_engine_through_trait_object_matches_direct_path_exactly() {
        let (rocket, motor, env) = reference_flight();
        let config = SimConfig::default();

        let direct = {
            let result = simulate_vertical(&rocket, &motor, &env, &config);
            SimSummary::from_result(&result, &rocket, &motor, &env, &config)
        };
        let engine: Box<dyn SimEngine> = Box::new(NativeEngine);
        let via_trait = engine.run(&rocket, &motor, &env, &config).unwrap();

        // Byte-identical, not approximately equal — determinism is sacred.
        assert_eq!(
            serde_json::to_string(&direct).unwrap(),
            serde_json::to_string(&via_trait).unwrap()
        );
    }

    /// A second engine registering and being selected through the same seam,
    /// standing in for the RocketPy/OpenRocket bridges of later steps.
    struct MockEngine;

    impl SimEngine for MockEngine {
        fn id(&self) -> &'static str {
            "mock-bridge"
        }
        fn version(&self) -> &'static str {
            "0.0.0-test"
        }
        fn run(
            &self,
            _rocket: &Rocket,
            _motor: &Motor,
            _env: &Environment,
            _config: &SimConfig,
        ) -> Result<SimSummary, String> {
            Err("mock engine has no solver".into())
        }
    }

    #[test]
    fn second_engine_can_register_and_be_selected_by_id() {
        let mut pool = engines();
        pool.push(Box::new(MockEngine));

        let picked = pool
            .iter()
            .find(|e| e.id() == "mock-bridge")
            .expect("mock engine must be selectable by id");
        assert_eq!(picked.version(), "0.0.0-test");

        let native = pool
            .iter()
            .find(|e| e.id() == "ascent-native")
            .expect("native engine must always be present");
        assert!(!native.version().is_empty());
    }

    #[test]
    fn factory_lists_native_engine_first() {
        let pool = engines();
        assert_eq!(pool[0].id(), "ascent-native");
    }
}
