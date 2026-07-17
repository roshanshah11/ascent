//! Acceptance tests for the 3-DOF planar solver (v0.2 Step 5).

use ascent_domain::Motor;
use ascent_sim::{
    planar_convergence, simulate_planar, simulate_vertical, AtmosphereModel, DragModel,
    Environment, PlanarVehicle, Recovery, Rocket, SimConfig, WindProfile,
};

fn c6() -> Motor {
    Motor::from_json(include_str!("../../ascent-domain/data/motors/estes_c6.json"))
        .expect("bundled C6 must parse")
}

fn alpha_iii() -> (Rocket, Environment) {
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
    (rocket, env)
}

/// Alpha III-ish rigid-body numbers: stable ~1.5 calibers, CP aft of CG.
fn stable_vehicle() -> PlanarVehicle {
    PlanarVehicle {
        cn_alpha_per_rad: 10.0,
        cp_from_nose_m: 0.22,
        cg_from_nose_m: 0.18,
        pitch_inertia_kgm2: 4.0e-4,
        reference_area_m2: std::f64::consts::PI * 0.0125 * 0.0125,
        launch_angle_rad: 0.0,
    }
}

/// Fins off: CP jumps ahead of the CG (nose-only Barrowman), margin < 0.
fn finless_vehicle() -> PlanarVehicle {
    PlanarVehicle {
        cn_alpha_per_rad: 2.0,
        cp_from_nose_m: 0.08,
        cg_from_nose_m: 0.18,
        pitch_inertia_kgm2: 4.0e-4,
        reference_area_m2: std::f64::consts::PI * 0.0125 * 0.0125,
        launch_angle_rad: 0.0,
    }
}

#[test]
fn calm_wind_planar_reproduces_the_vertical_solver_to_1e9() {
    let (rocket, env) = alpha_iii();
    let motor = c6();
    let config = SimConfig::default();

    let vertical = simulate_vertical(&rocket, &motor, &env, &config);
    let planar = simulate_planar(
        &rocket,
        &motor,
        &env,
        &stable_vehicle(),
        &WindProfile::calm(),
        &config,
    );

    assert!(
        (planar.apogee_m - vertical.apogee_m).abs() < 1e-9,
        "calm-wind planar apogee {} must equal vertical {} to 1e-9",
        planar.apogee_m,
        vertical.apogee_m
    );
    assert!(
        (planar.apogee_time_s - vertical.apogee_time_s).abs() < 1e-9,
        "apogee times must agree"
    );
    assert!(
        planar.landing_range_m.abs() < 1e-9,
        "no wind, no drift: landing range was {}",
        planar.landing_range_m
    );
    assert!(planar.max_aoa_deg.abs() < 1e-6, "vertical flight has zero AoA");
}

#[test]
fn crosswind_drift_is_sign_correct_and_matches_ballistic_hand_calc() {
    let (rocket, env) = alpha_iii();
    let motor = c6();
    let config = SimConfig::default();
    let wind_speed = 5.0;
    let wind = WindProfile::constant(wind_speed);

    let planar = simulate_planar(&rocket, &motor, &env, &stable_vehicle(), &wind, &config);
    let calm = simulate_planar(
        &rocket,
        &motor,
        &env,
        &stable_vehicle(),
        &WindProfile::calm(),
        &config,
    );

    // Weathercocking turns the nose INTO the wind (upwind = −x for +x wind).
    assert!(
        planar.weathercock_deg < -0.5,
        "nose should tilt upwind after rail exit, got {}°",
        planar.weathercock_deg
    );
    assert!(planar.max_aoa_deg.abs() > 0.5, "wind must produce a real AoA");

    // Under canopy the rocket drifts downwind. Hand calc: the chute reaches
    // wind speed in well under a second, so descent drift ≈ wind × descent
    // time; ascent moves it modestly upwind. Net drift must be downwind
    // (+x), bounded above by full-flight drift at wind speed.
    let descent_time = planar.landing_time_s - planar.apogee_time_s;
    let expected_descent_drift = wind_speed * descent_time;
    assert!(
        planar.landing_range_m > 0.4 * expected_descent_drift,
        "landing range {} m too small vs descent drift estimate {} m",
        planar.landing_range_m,
        expected_descent_drift
    );
    assert!(
        planar.landing_range_m < wind_speed * planar.landing_time_s,
        "landing range {} m cannot exceed drifting the whole flight at wind speed",
        planar.landing_range_m
    );
    // Wind costs a little apogee (the vehicle flies tilted, drag sees the
    // crosswind); it must not somehow gain altitude.
    assert!(planar.apogee_m <= calm.apogee_m + 1.0);
}

#[test]
fn finless_vehicle_with_cp_ahead_of_cg_goes_unstable() {
    let (rocket, env) = alpha_iii();
    let motor = c6();
    let config = SimConfig::default();
    let wind = WindProfile::constant(5.0);

    let stable = simulate_planar(&rocket, &motor, &env, &stable_vehicle(), &wind, &config);
    let unstable = simulate_planar(&rocket, &motor, &env, &finless_vehicle(), &wind, &config);

    assert!(
        stable.max_aoa_deg.abs() < 30.0,
        "stable vehicle should hold a small AoA, got {}°",
        stable.max_aoa_deg
    );
    assert!(
        unstable.max_aoa_deg.abs() > 45.0,
        "negative static margin must diverge, got {}°",
        unstable.max_aoa_deg
    );
    assert!(
        unstable.apogee_m < stable.apogee_m,
        "a tumbling rocket cannot outfly a stable one"
    );
}

#[test]
fn planar_convergence_is_green_at_default_dt() {
    let (rocket, env) = alpha_iii();
    let report = planar_convergence(
        &rocket,
        &c6(),
        &env,
        &stable_vehicle(),
        &WindProfile::constant(5.0),
        &SimConfig::default(),
    );
    assert!(
        report.converged,
        "planar solver must be timestep-converged at default dt: {:?}",
        report
    );
    // Landing range must also be stable under refinement (within a meter).
    assert!(
        (report.landing_range_m[0] - report.landing_range_m[1]).abs() < 1.0,
        "landing range not converged: {:?}",
        report.landing_range_m
    );
}

#[test]
fn layered_wind_profile_picks_the_layer_at_or_below_altitude() {
    let wind = ascent_sim::WindProfile {
        layers: vec![
            ascent_sim::WindLayer {
                altitude_m: 0.0,
                speed_ms: 2.0,
                direction_deg: 0.0,
            },
            ascent_sim::WindLayer {
                altitude_m: 100.0,
                speed_ms: 8.0,
                direction_deg: 180.0,
            },
        ],
    };
    assert!((wind.wind_x_at(50.0) - 2.0).abs() < 1e-12);
    assert!((wind.wind_x_at(100.0) - (-8.0)).abs() < 1e-12);
    assert!((wind.wind_x_at(500.0) - (-8.0)).abs() < 1e-12);
    assert_eq!(WindProfile::calm().wind_x_at(1000.0), 0.0);
}
