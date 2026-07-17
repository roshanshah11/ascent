//! Acceptance tests for Monte Carlo dispersion (v0.2 Step 6).

use ascent_domain::Motor;
use ascent_sim::{
    percentile, run_dispersion, simulate_planar, AtmosphereModel, Dispersion, DragModel,
    Environment, PlanarVehicle, Recovery, Rocket, SimConfig, Variation, VaryParam, WindProfile,
};

fn c6() -> Motor {
    Motor::from_json(include_str!("../../ascent-domain/data/motors/estes_c6.json"))
        .expect("bundled C6 must parse")
}

fn alpha_iii() -> (Rocket, Environment, PlanarVehicle) {
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
    let vehicle = PlanarVehicle {
        cn_alpha_per_rad: 10.0,
        cp_from_nose_m: 0.22,
        cg_from_nose_m: 0.18,
        pitch_inertia_kgm2: 4.0e-4,
        reference_area_m2: std::f64::consts::PI * 0.0125 * 0.0125,
        launch_angle_rad: 0.0,
    };
    (rocket, env, vehicle)
}

fn standard_vary() -> Vec<Variation> {
    vec![
        Variation { param: VaryParam::ThrustPct, sigma: 3.0 },
        Variation { param: VaryParam::CdPct, sigma: 5.0 },
        Variation { param: VaryParam::WindSpeedMs, sigma: 1.5 },
        Variation { param: VaryParam::LaunchAngleDeg, sigma: 2.0 },
        Variation { param: VaryParam::MassG, sigma: 1.0 },
    ]
}

#[test]
fn same_seed_produces_a_byte_identical_summary() {
    let (rocket, env, vehicle) = alpha_iii();
    let motor = c6();
    let wind = WindProfile::constant(3.0);
    let config = SimConfig::default();
    let spec = Dispersion { seed: 42, samples: 25, vary: standard_vary() };

    let a = run_dispersion(&rocket, &motor, &env, &vehicle, &wind, &config, &spec).unwrap();
    let b = run_dispersion(&rocket, &motor, &env, &vehicle, &wind, &config, &spec).unwrap();
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap(),
        "same seed must be byte-identical"
    );

    let c = run_dispersion(
        &rocket,
        &motor,
        &env,
        &vehicle,
        &wind,
        &config,
        &Dispersion { seed: 43, ..spec.clone() },
    )
    .unwrap();
    assert_ne!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&c).unwrap(),
        "a different seed must actually change the draw"
    );
}

#[test]
fn zero_sigma_dispersion_collapses_to_the_single_run() {
    let (rocket, env, vehicle) = alpha_iii();
    let motor = c6();
    let wind = WindProfile::constant(3.0);
    let config = SimConfig::default();
    let spec = Dispersion {
        seed: 7,
        samples: 10,
        vary: standard_vary().into_iter().map(|v| Variation { sigma: 0.0, ..v }).collect(),
    };

    let single = simulate_planar(&rocket, &motor, &env, &vehicle, &wind, &config);
    let summary = run_dispersion(&rocket, &motor, &env, &vehicle, &wind, &config, &spec).unwrap();

    for run in &summary.runs {
        assert_eq!(run.apogee_m, single.apogee_m, "zero sigma must not perturb apogee");
        assert_eq!(run.landing_range_m, single.landing_range_m);
    }
    assert_eq!(summary.apogee_p5_m, summary.apogee_p95_m);
    assert_eq!(summary.landing_ellipse.a_m, 0.0, "no spread, no ellipse");
}

#[test]
fn percentiles_match_the_hand_checked_ten_sample_fixture() {
    // Hand check (numpy-style linear interpolation on 1..=10):
    // p5  -> rank 0.45  -> 1 + 0.45*(2-1)  = 1.45
    // p50 -> rank 4.5   -> 5 + 0.5*(6-5)   = 5.5
    // p95 -> rank 8.55  -> 9 + 0.55*(10-9) = 9.55
    let values: Vec<f64> = (1..=10).map(|i| i as f64).collect();
    assert!((percentile(&values, 5.0) - 1.45).abs() < 1e-12);
    assert!((percentile(&values, 50.0) - 5.5).abs() < 1e-12);
    assert!((percentile(&values, 95.0) - 9.55).abs() < 1e-12);
    // Order-independence: percentile sorts internally.
    let shuffled = [7.0, 1.0, 9.0, 3.0, 5.0, 10.0, 2.0, 8.0, 4.0, 6.0];
    assert!((percentile(&shuffled, 50.0) - 5.5).abs() < 1e-12);
}

#[test]
fn dispersion_spreads_are_physically_sane() {
    let (rocket, env, vehicle) = alpha_iii();
    let motor = c6();
    let wind = WindProfile::constant(3.0);
    let config = SimConfig::default();
    let spec = Dispersion { seed: 1234, samples: 50, vary: standard_vary() };

    let s = run_dispersion(&rocket, &motor, &env, &vehicle, &wind, &config, &spec).unwrap();
    assert_eq!(s.runs.len(), 50);
    assert!(s.apogee_p5_m < s.apogee_p50_m && s.apogee_p50_m < s.apogee_p95_m);
    // Nominal apogee ~358 m; ±3% thrust / ±5% Cd should stay well inside ±25%.
    assert!(s.apogee_p50_m > 250.0 && s.apogee_p50_m < 450.0, "p50 {}", s.apogee_p50_m);
    assert!(s.apogee_p95_m - s.apogee_p5_m < 0.5 * s.apogee_p50_m, "spread implausibly wide");
    // 3 m/s mean wind: the fleet lands downwind on average, with real spread.
    assert!(s.landing_mean_m > 0.0, "mean landing {}", s.landing_mean_m);
    assert!(s.landing_ellipse.a_m > 0.0);
    assert_eq!(s.landing_ellipse.b_m, 0.0, "planar solver has no crossrange");
    // The summary is its own evidence: seed and distributions echoed back.
    assert_eq!(s.seed, 1234);
    assert_eq!(s.vary.len(), 5);
}

#[test]
fn zero_samples_is_an_error_not_a_panic() {
    let (rocket, env, vehicle) = alpha_iii();
    let spec = Dispersion { seed: 1, samples: 0, vary: vec![] };
    assert!(run_dispersion(
        &rocket,
        &c6(),
        &env,
        &vehicle,
        &WindProfile::calm(),
        &SimConfig::default(),
        &spec
    )
    .is_err());
}

/// Release-build perf gate (plan acceptance: 1000 samples < 5 s). Ignored in
/// normal (debug) runs; execute with:
/// `cargo test --release -p ascent-sim --test dispersion -- --ignored`
#[test]
#[ignore]
fn thousand_sample_run_finishes_under_five_seconds_release() {
    let (rocket, env, vehicle) = alpha_iii();
    let motor = c6();
    let wind = WindProfile::constant(3.0);
    let config = SimConfig::default();
    let spec = Dispersion { seed: 99, samples: 1000, vary: standard_vary() };

    let start = std::time::Instant::now();
    let s = run_dispersion(&rocket, &motor, &env, &vehicle, &wind, &config, &spec).unwrap();
    let elapsed = start.elapsed();
    assert_eq!(s.runs.len(), 1000);
    assert!(elapsed.as_secs_f64() < 5.0, "1000 samples took {:?}", elapsed);
}
