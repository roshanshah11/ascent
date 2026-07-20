//! Acceptance tests for dual-deploy recovery (v0.5 Step 2): drogue at
//! apogee, main at the configured altitude, in all three engines.

use ascent_domain::Motor;
use ascent_sim::{
    simulate_planar, simulate_planar_staged, simulate_vertical, AtmosphereModel, DragModel, Drogue,
    Environment, EventKind, PlanarStage, PlanarVehicle, Recovery, Rocket, SimConfig, WindProfile,
};

fn c6() -> Motor {
    Motor::from_json(include_str!(
        "../../ascent-domain/data/motors/estes_c6.json"
    ))
    .expect("bundled C6 must parse")
}

fn alpha_iii(recovery: Recovery) -> (Rocket, Environment) {
    let rocket = Rocket {
        name: "Estes Alpha III".into(),
        dry_mass_kg: 0.034,
        drag: Some(DragModel {
            cd: 0.60,
            reference_area_m2: std::f64::consts::PI * 0.0125 * 0.0125,
        }),
        recovery: Some(recovery),
    };
    let env = Environment {
        gravity_ms2: 9.80665,
        atmosphere: AtmosphereModel::Standard,
        rail_length_m: 0.9,
    };
    (rocket, env)
}

fn single() -> Recovery {
    Recovery::single(0.75, std::f64::consts::PI * 0.15 * 0.15)
}

fn dual(main_deploy_altitude_m: f64) -> Recovery {
    Recovery {
        drogue: Some(Drogue {
            cd: 0.8,
            area_m2: std::f64::consts::PI * 0.04 * 0.04,
        }),
        main_deploy_altitude_m: Some(main_deploy_altitude_m),
        ..single()
    }
}

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

// ---- vertical engine ----

#[test]
fn vertical_main_deploys_at_the_configured_altitude() {
    let (rocket, env) = alpha_iii(dual(60.0));
    let result = simulate_vertical(&rocket, &c6(), &env, &SimConfig::default());

    let deploy = result
        .event(EventKind::MainDeploy)
        .expect("dual-deploy flight must emit MainDeploy");
    assert!(
        (deploy.altitude_m - 60.0).abs() < 1e-9,
        "main deploys at 60 m, got {} m",
        deploy.altitude_m
    );
    let apogee = result.event(EventKind::Apogee).unwrap();
    assert!(deploy.t > apogee.t, "main deploy happens during descent");
    // Under drogue the vehicle falls much faster than it lands under main.
    assert!(
        deploy.velocity_ms.abs() > 2.0 * result.landing_velocity_ms.abs(),
        "drogue descent ({} m/s) should be much faster than main landing ({} m/s)",
        deploy.velocity_ms,
        result.landing_velocity_ms
    );
}

#[test]
fn vertical_single_deploy_never_emits_main_deploy() {
    let (rocket, env) = alpha_iii(single());
    let result = simulate_vertical(&rocket, &c6(), &env, &SimConfig::default());
    assert!(result.event(EventKind::MainDeploy).is_none());
    assert!(result.event(EventKind::RecoveryDeploy).is_some());
}

#[test]
fn vertical_dual_deploy_lands_sooner_than_single_deploy() {
    // Drogue descent from apogee to 60 m is fast; a single main from
    // apogee drifts down slowly the whole way.
    let (dual_rocket, env) = alpha_iii(dual(60.0));
    let (single_rocket, _) = alpha_iii(single());
    let motor = c6();
    let config = SimConfig::default();
    let dual_run = simulate_vertical(&dual_rocket, &motor, &env, &config);
    let single_run = simulate_vertical(&single_rocket, &motor, &env, &config);
    assert!(
        dual_run.landing_time_s < single_run.landing_time_s,
        "dual {} s vs single {} s",
        dual_run.landing_time_s,
        single_run.landing_time_s
    );
    // Same main, so touchdown speeds agree closely.
    assert!(
        (dual_run.landing_velocity_ms - single_run.landing_velocity_ms).abs() < 0.2,
        "dual lands at {} m/s, single at {} m/s",
        dual_run.landing_velocity_ms,
        single_run.landing_velocity_ms
    );
}

#[test]
fn apogee_below_deploy_altitude_opens_main_at_apogee() {
    // Deploy altitude far above apogee, no drogue: physically identical
    // to single-deploy, and MainDeploy stamps at apogee.
    let recovery = Recovery {
        main_deploy_altitude_m: Some(10_000.0),
        ..single()
    };
    let (rocket, env) = alpha_iii(recovery);
    let (single_rocket, _) = alpha_iii(single());
    let motor = c6();
    let config = SimConfig::default();

    let run = simulate_vertical(&rocket, &motor, &env, &config);
    let reference = simulate_vertical(&single_rocket, &motor, &env, &config);

    let deploy = run.event(EventKind::MainDeploy).unwrap();
    let apogee = run.event(EventKind::Apogee).unwrap();
    assert!((deploy.t - apogee.t).abs() < 1e-12);
    assert!((run.apogee_m - reference.apogee_m).abs() < 1e-9);
    assert!((run.landing_time_s - reference.landing_time_s).abs() < 1e-9);
}

// ---- planar engine ----

#[test]
fn planar_dual_deploy_cuts_wind_drift() {
    let motor = c6();
    let config = SimConfig::default();
    let wind = WindProfile::constant(5.0);
    let vehicle = stable_vehicle();

    let (dual_rocket, env) = alpha_iii(dual(60.0));
    let (single_rocket, _) = alpha_iii(single());
    let dual_run = simulate_planar(&dual_rocket, &motor, &env, &vehicle, &wind, &config);
    let single_run = simulate_planar(&single_rocket, &motor, &env, &vehicle, &wind, &config);

    assert!(
        dual_run.landing_range_m < single_run.landing_range_m,
        "drogue descent must drift less: dual {} m vs single {} m",
        dual_run.landing_range_m,
        single_run.landing_range_m
    );
    // Sanity: both still drift downwind.
    assert!(dual_run.landing_range_m > 0.0);
}

#[test]
fn staged_single_stage_dual_deploy_matches_simulate_planar_to_1e9() {
    let motor = c6();
    let config = SimConfig::default();
    let wind = WindProfile::constant(4.0);
    let vehicle = stable_vehicle();
    let (rocket, env) = alpha_iii(dual(60.0));

    let direct = simulate_planar(&rocket, &motor, &env, &vehicle, &wind, &config);
    let staged = simulate_planar_staged(
        &[PlanarStage {
            dry_mass_kg: rocket.dry_mass_kg,
            motor: motor.clone(),
            separation_delay_s: 0.0,
            vehicle: vehicle.clone(),
            drag: rocket.drag.clone(),
        }],
        rocket.recovery.clone(),
        &env,
        &wind,
        &config,
    );

    assert!((staged.summary.apogee_m - direct.apogee_m).abs() < 1e-9);
    assert!((staged.summary.landing_time_s - direct.landing_time_s).abs() < 1e-9);
    assert!((staged.summary.landing_range_m - direct.landing_range_m).abs() < 1e-9);
    let deploy = staged
        .events
        .iter()
        .find(|e| e.kind == "MainDeploy")
        .expect("staged timeline records MainDeploy");
    assert!((deploy.altitude_m - 60.0).abs() < 1e-9);
}

// ---- sixdof engine ----

#[test]
fn sixdof_main_deploys_at_the_configured_altitude() {
    use ascent_aero::{fin_set_cn, nose_cn, total_cp_from_nose_m, Vehicle as AeroVehicle};
    use ascent_domain::vehicle::reference_vehicle;
    use ascent_sim::{SixDofEngine, SixDofLaunch, SixDofVehicle, Wind3DProfile};

    let tree = reference_vehicle();
    let props = tree.mass_properties();
    let (aero, shape) = AeroVehicle::from_tree(&tree).unwrap();
    let vehicle = SixDofVehicle {
        cg_from_nose_m: props.cg_from_nose_m,
        pitch_yaw_inertia_kgm2: props.longitudinal_moi_kg_m2,
        cp_from_nose_m: total_cp_from_nose_m(&aero, shape),
        cn_alpha_per_rad: nose_cn(shape, aero.nose.length_m).cn_alpha + fin_set_cn(&aero).cn_alpha,
        reference_area_m2: std::f64::consts::PI * (aero.diameter_m() / 2.0).powi(2),
    };
    let engine =
        SixDofEngine::new(vehicle, Wind3DProfile::calm(), SixDofLaunch::vertical()).unwrap();

    let (rocket, env) = alpha_iii(dual(60.0));
    let result = engine
        .run_detailed(&rocket, &c6(), &env, &SimConfig::default())
        .unwrap();
    let deploy = result
        .summary
        .events
        .iter()
        .find(|e| e.kind == "MainDeploy")
        .expect("sixdof dual-deploy flight must emit MainDeploy");
    assert!(
        (deploy.altitude_m - 60.0).abs() < 1e-9,
        "main deploys at 60 m, got {} m",
        deploy.altitude_m
    );
}

// ---- serde compatibility ----

#[test]
fn pre_v05_recovery_json_round_trips_unchanged() {
    // Old documents carry only the two original fields …
    let old = r#"{"chute_cd":0.75,"chute_area_m2":0.07}"#;
    let recovery: Recovery = serde_json::from_str(old).expect("pre-v0.5 JSON deserializes");
    assert!(recovery.drogue.is_none());
    assert!(recovery.main_deploy_altitude_m.is_none());
    // … and single-deploy re-serializes without the new keys, so every
    // stored hash over a Recovery stays byte-stable.
    let json = serde_json::to_string(&recovery).unwrap();
    assert!(!json.contains("drogue"));
    assert!(!json.contains("main_deploy_altitude_m"));
}
