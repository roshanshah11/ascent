//! Multi-stage flight: variable-mass handoff across stages in the planar
//! 3-DOF and 6-DOF engines. The fixture mirrors the domain's two-stage
//! reference vehicle (D12 booster, C6 sustainer) with hand-set rigid-body
//! numbers — engines take plain numbers; the tree→params derivation is
//! covered in ascent-aero/ascent-app.

use ascent_domain::Motor;
use ascent_sim::{
    simulate_planar_staged, simulate_sixdof_staged, DragModel, Environment, PlanarStage,
    PlanarVehicle, Recovery, Rocket, SimConfig, SixDofLaunch, SixDofStage, SixDofVehicle,
    Wind3DProfile, WindProfile,
};

fn c6() -> Motor {
    Motor::from_json(include_str!(
        "../../ascent-domain/data/motors/estes_c6.json"
    ))
    .unwrap()
}

fn d12() -> Motor {
    Motor::from_json(include_str!(
        "../../ascent-domain/data/motors/estes_d12.json"
    ))
    .unwrap()
}

fn vehicle(cg: f64, cp: f64, inertia: f64) -> PlanarVehicle {
    PlanarVehicle {
        cn_alpha_per_rad: 8.0,
        cp_from_nose_m: cp,
        cg_from_nose_m: cg,
        pitch_inertia_kgm2: inertia,
        reference_area_m2: 4.9e-4,
        launch_angle_rad: 0.0,
    }
}

fn drag(cd: f64) -> Option<DragModel> {
    Some(DragModel {
        cd,
        reference_area_m2: 4.9e-4,
    })
}

/// Booster carries the full stack; sustainer flies alone after separation.
fn two_stages() -> Vec<PlanarStage> {
    vec![
        PlanarStage {
            // Full stack minus what leaves at separation is implied by the
            // next stage's dry mass; this stage's dry mass is what drops.
            dry_mass_kg: 0.027,
            motor: d12(),
            separation_delay_s: 0.0,
            vehicle: vehicle(0.26, 0.32, 6.0e-4),
            drag: drag(0.65),
        },
        PlanarStage {
            dry_mass_kg: 0.034,
            motor: c6(),
            separation_delay_s: 0.0,
            vehicle: vehicle(0.17, 0.21, 2.6e-4),
            drag: drag(0.60),
        },
    ]
}

fn recovery() -> Recovery {
    Recovery {
        chute_cd: 0.75,
        chute_area_m2: std::f64::consts::PI * 0.15 * 0.15,
        drogue: None,
        main_deploy_altitude_m: None,
    }
}

fn config() -> SimConfig {
    SimConfig {
        dt_s: 0.001,
        max_time_s: 240.0,
    }
}

#[test]
fn two_stage_planar_flies_end_to_end_with_separation_events() {
    let result = simulate_planar_staged(
        &two_stages(),
        Some(recovery()),
        &Environment::default(),
        &WindProfile::calm(),
        &config(),
    );
    let kinds: Vec<&str> = result.events.iter().map(|e| e.kind.as_str()).collect();
    // Booster burnout → separation → sustainer ignition, in that order.
    let sep = kinds.iter().position(|k| *k == "StageSeparation").unwrap();
    let ign = kinds.iter().position(|k| *k == "StageIgnition").unwrap();
    let burnout = kinds.iter().position(|k| *k == "Burnout").unwrap();
    assert!(burnout < sep, "burnout before separation: {kinds:?}");
    assert!(sep < ign, "separation before ignition: {kinds:?}");
    // Times are monotonic and the flight lands.
    let times: Vec<f64> = result.events.iter().map(|e| e.t_s).collect();
    assert!(
        times.windows(2).all(|w| w[0] <= w[1]),
        "monotonic: {times:?}"
    );
    assert!(result.summary.apogee_m > 0.0);
    assert!(result.summary.landing_time_s > result.summary.apogee_time_s);
}

#[test]
fn separation_happens_at_booster_burnout_plus_delay() {
    let mut stages = two_stages();
    stages[0].separation_delay_s = 0.4;
    let result = simulate_planar_staged(
        &stages,
        Some(recovery()),
        &Environment::default(),
        &WindProfile::calm(),
        &config(),
    );
    let burnout = result
        .events
        .iter()
        .find(|e| e.kind == "Burnout")
        .unwrap()
        .t_s;
    let sep = result
        .events
        .iter()
        .find(|e| e.kind == "StageSeparation")
        .unwrap()
        .t_s;
    assert!(
        (sep - (burnout + 0.4)).abs() < 1e-9,
        "sep {sep} != burnout {burnout} + 0.4"
    );
}

#[test]
fn two_stage_apogee_beats_the_sustainer_alone() {
    let staged = simulate_planar_staged(
        &two_stages(),
        Some(recovery()),
        &Environment::default(),
        &WindProfile::calm(),
        &config(),
    );
    let solo = simulate_planar_staged(
        &two_stages()[1..],
        Some(recovery()),
        &Environment::default(),
        &WindProfile::calm(),
        &config(),
    );
    assert!(
        staged.summary.apogee_m > solo.summary.apogee_m,
        "staged {} <= solo {}",
        staged.summary.apogee_m,
        solo.summary.apogee_m
    );
}

#[test]
fn single_stage_staged_run_matches_simulate_planar() {
    use ascent_sim::simulate_planar;
    let stages = two_stages()[1..].to_vec();
    let staged = simulate_planar_staged(
        &stages,
        Some(recovery()),
        &Environment::default(),
        &WindProfile::calm(),
        &config(),
    );
    let rocket = Rocket {
        name: "solo".into(),
        dry_mass_kg: stages[0].dry_mass_kg,
        drag: stages[0].drag.clone(),
        recovery: Some(recovery()),
    };
    let plain = simulate_planar(
        &rocket,
        &stages[0].motor,
        &Environment::default(),
        &stages[0].vehicle,
        &WindProfile::calm(),
        &config(),
    );
    assert!((staged.summary.apogee_m - plain.apogee_m).abs() < 1e-9);
    assert!((staged.summary.landing_time_s - plain.landing_time_s).abs() < 1e-9);
}

fn sixdof_stages() -> Vec<SixDofStage> {
    let v = |cg: f64, cp: f64, inertia: f64| SixDofVehicle {
        cg_from_nose_m: cg,
        pitch_yaw_inertia_kgm2: inertia,
        cp_from_nose_m: cp,
        cn_alpha_per_rad: 8.0,
        reference_area_m2: 4.9e-4,
    };
    vec![
        SixDofStage {
            dry_mass_kg: 0.027,
            motor: d12(),
            separation_delay_s: 0.0,
            vehicle: v(0.26, 0.32, 6.0e-4),
            drag: drag(0.65),
        },
        SixDofStage {
            dry_mass_kg: 0.034,
            motor: c6(),
            separation_delay_s: 0.0,
            vehicle: v(0.17, 0.21, 2.6e-4),
            drag: drag(0.60),
        },
    ]
}

#[test]
fn two_stage_sixdof_flies_end_to_end_with_separation_events() {
    let result = simulate_sixdof_staged(
        &sixdof_stages(),
        Some(recovery()),
        &Wind3DProfile::calm(),
        &SixDofLaunch::vertical(),
        &Environment::default(),
        &config(),
    )
    .unwrap();
    let kinds: Vec<&str> = result
        .summary
        .events
        .iter()
        .map(|e| e.kind.as_str())
        .collect();
    let sep = kinds.iter().position(|k| *k == "StageSeparation").unwrap();
    let ign = kinds.iter().position(|k| *k == "StageIgnition").unwrap();
    let burnout = kinds.iter().position(|k| *k == "Burnout").unwrap();
    assert!(burnout < sep && sep < ign, "order: {kinds:?}");
    assert!(
        kinds.contains(&"Apogee") && kinds.contains(&"Landing"),
        "{kinds:?}"
    );
    let times: Vec<f64> = result.summary.events.iter().map(|e| e.t_s).collect();
    assert!(
        times.windows(2).all(|w| w[0] <= w[1]),
        "monotonic: {times:?}"
    );
    assert!(result.summary.apogee_m > 0.0);
}

#[test]
fn sixdof_staged_apogee_beats_sustainer_alone_and_matches_planar_class() {
    let env = Environment::default();
    let cfg = config();
    let staged = simulate_sixdof_staged(
        &sixdof_stages(),
        Some(recovery()),
        &Wind3DProfile::calm(),
        &SixDofLaunch::vertical(),
        &env,
        &cfg,
    )
    .unwrap();
    let solo = simulate_sixdof_staged(
        &sixdof_stages()[1..],
        Some(recovery()),
        &Wind3DProfile::calm(),
        &SixDofLaunch::vertical(),
        &env,
        &cfg,
    )
    .unwrap();
    assert!(staged.summary.apogee_m > solo.summary.apogee_m);
    // Cross-engine consistency: staged 6-DOF apogee within 5% of the
    // staged planar apogee for the same calm-air two-stage vehicle.
    let planar = simulate_planar_staged(
        &two_stages(),
        Some(recovery()),
        &env,
        &WindProfile::calm(),
        &cfg,
    );
    let rel = (staged.summary.apogee_m - planar.summary.apogee_m).abs() / planar.summary.apogee_m;
    assert!(
        rel < 0.05,
        "6dof {} vs planar {} ({}%)",
        staged.summary.apogee_m,
        planar.summary.apogee_m,
        rel * 100.0
    );
}

#[test]
fn sixdof_staged_is_deterministic() {
    let run = || {
        simulate_sixdof_staged(
            &sixdof_stages(),
            Some(recovery()),
            &Wind3DProfile::calm(),
            &SixDofLaunch::vertical(),
            &Environment::default(),
            &config(),
        )
        .unwrap()
    };
    let (a, b) = (run(), run());
    assert_eq!(
        serde_json::to_string(&a.summary).unwrap(),
        serde_json::to_string(&b.summary).unwrap()
    );
}

#[test]
fn staged_run_is_deterministic() {
    let a = simulate_planar_staged(
        &two_stages(),
        Some(recovery()),
        &Environment::default(),
        &WindProfile::calm(),
        &config(),
    );
    let b = simulate_planar_staged(
        &two_stages(),
        Some(recovery()),
        &Environment::default(),
        &WindProfile::calm(),
        &config(),
    );
    assert_eq!(
        serde_json::to_string(&a.summary).unwrap(),
        serde_json::to_string(&b.summary).unwrap()
    );
    assert_eq!(
        serde_json::to_string(&a.events).unwrap(),
        serde_json::to_string(&b.events).unwrap()
    );
}
