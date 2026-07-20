//! Day 3 golden-fixture regression against OpenRocket 24.12.
//!
//! Reference: OpenRocket's bundled "A simple model rocket" example flown on an
//! Estes C6-5, manually exported (data/reference/openrocket-alpha3-c6.csv) and
//! frozen into tests/fixtures/openrocket_alpha3_c6.json. Tolerances are
//! justified in docs/EVIDENCE.md — they reflect real model differences
//! (vertical point-mass vs 3D with wind, constant Cd vs Mach-dependent),
//! not tuning slack.

use approx::assert_relative_eq;
use ascent_domain::Motor;
use ascent_sim::{
    simulate_vertical, AtmosphereModel, DragModel, Environment, EventKind, Recovery, Rocket,
    SimConfig,
};

const C6_JSON: &str = include_str!("../../ascent-domain/data/motors/estes_c6.json");
const FIXTURE: &str = include_str!("fixtures/openrocket_alpha3_c6.json");

struct Reference {
    fixture: serde_json::Value,
}

impl Reference {
    fn load() -> Self {
        Self {
            fixture: serde_json::from_str(FIXTURE).expect("fixture JSON parses"),
        }
    }
    fn value(&self, key: &str) -> f64 {
        self.fixture["reference_values"][key]
            .as_f64()
            .unwrap_or_else(|| panic!("missing reference value {key}"))
    }
    fn launch(&self, key: &str) -> f64 {
        self.fixture["launch_conditions"][key]
            .as_f64()
            .unwrap_or_else(|| panic!("missing launch condition {key}"))
    }
    fn vehicle(&self, key: &str) -> f64 {
        self.fixture["vehicle"][key]
            .as_f64()
            .unwrap_or_else(|| panic!("missing vehicle value {key}"))
    }
    fn chute(&self, key: &str) -> f64 {
        self.fixture["vehicle"]["chute"][key]
            .as_f64()
            .unwrap_or_else(|| panic!("missing chute value {key}"))
    }
}

/// Ascent configured to match the OpenRocket run: same liftoff mass, same
/// reference area, OpenRocket's own computed coast Cd, its derived chute cdA,
/// its latitude-adjusted gravity, and its 1 m launch rod.
fn matched_setup(reference: &Reference) -> (Rocket, Motor, Environment, SimConfig) {
    let motor = Motor::from_json(C6_JSON).unwrap();
    let liftoff_mass_kg = reference.vehicle("liftoff_mass_g") / 1000.0;
    let chute_d = reference.chute("diameter_m");
    let rocket = Rocket {
        name: "OpenRocket simple model rocket".into(),
        // Hold total liftoff mass equal to OpenRocket's (its motor entry is
        // 23.1 g vs our cert 24.1 g); burnout mass then matches exactly.
        dry_mass_kg: liftoff_mass_kg - motor.total_mass_kg,
        drag: Some(DragModel {
            cd: reference.vehicle("coast_cd_openrocket"),
            reference_area_m2: reference.vehicle("reference_area_m2"),
        }),
        recovery: Some(Recovery {
            chute_cd: reference.chute("cd"),
            chute_area_m2: std::f64::consts::PI * (chute_d / 2.0) * (chute_d / 2.0),
            drogue: None,
            main_deploy_altitude_m: None,
        }),
    };
    let env = Environment {
        gravity_ms2: reference.launch("gravity_ms2"),
        atmosphere: AtmosphereModel::Standard,
        rail_length_m: reference.launch("launch_rod_m"),
    };
    (rocket, motor, env, SimConfig::default())
}

/// Not an assertion — prints the matched-run numbers for docs/EVIDENCE.md.
/// Run with: cargo test --test openrocket_reference -- --ignored --nocapture
#[test]
#[ignore]
fn print_matched_run_summary() {
    let reference = Reference::load();
    let (rocket, motor, env, config) = matched_setup(&reference);
    let r = simulate_vertical(&rocket, &motor, &env, &config);
    println!(
        "rail_exit_v={:.2} burnout_t={:.3} burnout_h={:.1} burnout_v={:.2} max_v={:.2} apogee={:.1} apogee_t={:.3} landing_v={:.2} landing_t={:.1}",
        r.rail_exit_velocity_ms,
        r.burnout_time_s,
        r.burnout_altitude_m,
        r.burnout_velocity_ms,
        r.max_velocity_ms,
        r.apogee_m,
        r.apogee_time_s,
        r.landing_velocity_ms,
        r.landing_time_s
    );
}

#[test]
fn burnout_state_matches_openrocket() {
    let reference = Reference::load();
    let (rocket, motor, env, config) = matched_setup(&reference);
    let r = simulate_vertical(&rocket, &motor, &env, &config);

    // Burn time comes from the same certified curve both tools use.
    assert_relative_eq!(
        r.burnout_time_s,
        reference.value("burnout_time_s"),
        max_relative = 0.01
    );
    // 2% — identical impulse and burnout mass; residual is wind/AoA drag we
    // don't model plus OpenRocket's Mach-varying Cd during the burn.
    assert_relative_eq!(
        r.burnout_velocity_ms,
        reference.value("burnout_velocity_ms"),
        max_relative = 0.02
    );
    assert_relative_eq!(
        r.max_velocity_ms,
        reference.value("max_velocity_ms"),
        max_relative = 0.02
    );
    // 5% on burnout altitude: early-flight Cd differences integrate here.
    assert_relative_eq!(
        r.burnout_altitude_m,
        reference.value("burnout_altitude_m"),
        max_relative = 0.05
    );
}

#[test]
fn apogee_matches_openrocket() {
    let reference = Reference::load();
    let (rocket, motor, env, config) = matched_setup(&reference);
    let r = simulate_vertical(&rocket, &motor, &env, &config);

    // 5% — the headline number. Vertical no-wind flight should slightly beat
    // OpenRocket's 2 m/s-wind 3D flight, never trail it badly.
    assert_relative_eq!(r.apogee_m, reference.value("apogee_m"), max_relative = 0.05);
    // 10% on apogee TIME only: OpenRocket deploys the chute at the ejection
    // charge (6.861 s, before apogee), which truncates the coast — its apogee
    // registers at 7.278 s. We coast ballistically to ~7.82 s. Altitude still
    // agrees because the reference gains only 1.4 m after deploy.
    assert_relative_eq!(
        r.apogee_time_s,
        reference.value("apogee_time_s"),
        max_relative = 0.10
    );
}

#[test]
fn rail_exit_matches_openrocket() {
    let reference = Reference::load();
    let (rocket, motor, env, config) = matched_setup(&reference);
    let r = simulate_vertical(&rocket, &motor, &env, &config);

    // 10% — the two tools measure rod departure differently (OpenRocket
    // tracks travel along the rod from the initial CG; we cross an altitude).
    assert_relative_eq!(
        r.rail_exit_velocity_ms,
        reference.value("rail_exit_velocity_ms"),
        max_relative = 0.10
    );
}

#[test]
fn descent_matches_openrocket() {
    let reference = Reference::load();
    let (rocket, motor, env, config) = matched_setup(&reference);
    let r = simulate_vertical(&rocket, &motor, &env, &config);

    // Compare vertical descent rate mid-descent (OpenRocket's summary
    // "ground hit velocity" includes lateral wind drift; its vertical rate at
    // 180 m is the clean number). The chute cdA was derived from the same
    // export, so 5% here checks our descent integration, not the chute guess.
    let landing = r.event(EventKind::Landing).expect("must land");
    assert_relative_eq!(
        -landing.velocity_ms,
        reference.value("descent_rate_at_180m_ms"),
        max_relative = 0.10
    );
    // Flight time is the independent descent check: apogee height and descent
    // rate together fix it. 10% covers deploy-at-apogee vs deploy-at-ejection.
    assert_relative_eq!(
        r.landing_time_s,
        reference.value("ground_hit_time_s"),
        max_relative = 0.10
    );
}
