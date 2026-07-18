use ascent_aero::{fin_set_cn, nose_cn, total_cp_from_nose_m, Vehicle as AeroVehicle};
use ascent_domain::{vehicle::reference_vehicle, Motor};
use ascent_sim::{
    simulate_planar, simulate_vertical, AtmosphereModel, DragModel, Environment, FlightPhase,
    PlanarVehicle, Recovery, Rocket, SimConfig, SimEngine, SixDofEngine, SixDofLaunch,
    SixDofVehicle, Wind3DLayer, Wind3DProfile, WindProfile,
};

fn c6() -> Motor {
    Motor::from_json(include_str!(
        "../../ascent-domain/data/motors/estes_c6.json"
    ))
    .expect("bundled C6 must parse")
}

fn alpha_iii() -> (Rocket, Environment) {
    let radius = 0.0125_f64;
    (
        Rocket {
            name: "Estes Alpha III".into(),
            dry_mass_kg: 0.034,
            drag: Some(DragModel {
                cd: 0.60,
                reference_area_m2: std::f64::consts::PI * radius * radius,
            }),
            recovery: Some(Recovery {
                chute_cd: 0.75,
                chute_area_m2: std::f64::consts::PI * 0.15 * 0.15,
            }),
        },
        Environment {
            gravity_ms2: 9.80665,
            atmosphere: AtmosphereModel::Standard,
            rail_length_m: 0.9,
        },
    )
}

fn tree_vehicle() -> SixDofVehicle {
    let tree = reference_vehicle();
    let props = tree.mass_properties();
    let (aero, shape) = AeroVehicle::from_tree(&tree).expect("reference tree flattens");
    let nose = nose_cn(shape, aero.nose.length_m);
    let fins = fin_set_cn(&aero);
    SixDofVehicle {
        cg_from_nose_m: props.cg_from_nose_m,
        pitch_yaw_inertia_kgm2: props.longitudinal_moi_kg_m2,
        cp_from_nose_m: total_cp_from_nose_m(&aero, shape),
        cn_alpha_per_rad: nose.cn_alpha + fins.cn_alpha,
        reference_area_m2: std::f64::consts::PI * (aero.diameter_m() / 2.0).powi(2),
    }
}

fn configured_engine() -> SixDofEngine {
    SixDofEngine::new(
        tree_vehicle(),
        Wind3DProfile::calm(),
        SixDofLaunch::vertical(),
    )
    .expect("tree-derived configuration is valid")
}

fn engine_for(wind_x_ms: f64, tilt_rad: f64) -> SixDofEngine {
    let wind = if wind_x_ms == 0.0 {
        Wind3DProfile::calm()
    } else {
        Wind3DProfile {
            layers: vec![Wind3DLayer {
                altitude_m: 0.0,
                velocity_ms: [wind_x_ms, 0.0, 0.0],
            }],
        }
    };
    SixDofEngine::new(
        tree_vehicle(),
        wind,
        SixDofLaunch {
            tilt_rad,
            azimuth_rad: 0.0,
        },
    )
    .unwrap()
}

fn planar_vehicle(launch_angle_rad: f64) -> PlanarVehicle {
    let v = tree_vehicle();
    PlanarVehicle {
        cn_alpha_per_rad: v.cn_alpha_per_rad,
        cp_from_nose_m: v.cp_from_nose_m,
        cg_from_nose_m: v.cg_from_nose_m,
        pitch_inertia_kgm2: v.pitch_yaw_inertia_kgm2,
        reference_area_m2: v.reference_area_m2,
        launch_angle_rad,
    }
}

fn quaternion_norm(q: [f64; 4]) -> f64 {
    q.iter().map(|v| v * v).sum::<f64>().sqrt()
}

fn body_axis_tilt_deg(q: [f64; 4]) -> f64 {
    let [_w, x, y, _z] = q;
    let inertial_z_component = 1.0 - 2.0 * (x * x + y * y);
    inertial_z_component.clamp(-1.0, 1.0).acos().to_degrees()
}

#[test]
fn configured_sixdof_engine_runs_through_the_standard_trait_seam() {
    fn requires_engine<T: SimEngine>() {}
    requires_engine::<SixDofEngine>();

    let engine = configured_engine();
    assert_eq!(engine.id(), "ascent-sixdof");
    assert!(!engine.version().is_empty());

    let (rocket, env) = alpha_iii();
    let summary = (&engine as &dyn SimEngine)
        .run(&rocket, &c6(), &env, &SimConfig::default())
        .expect("configured engine runs through SimEngine");
    assert!(summary.apogee_m.is_finite());
    assert_eq!(summary.rocket, "Estes Alpha III");
}

#[test]
fn calm_vertical_detailed_history_matches_native_and_is_byte_deterministic() {
    let engine = configured_engine();
    let (rocket, env) = alpha_iii();
    let motor = c6();
    let config = SimConfig::default();
    let native = simulate_vertical(&rocket, &motor, &env, &config);

    let first = engine.run_detailed(&rocket, &motor, &env, &config).unwrap();
    let second = engine.run_detailed(&rocket, &motor, &env, &config).unwrap();
    let relative = (first.summary.apogee_m - native.apogee_m).abs() / native.apogee_m;
    assert!(
        relative <= 1e-6,
        "calm apogee relative residual {relative:e}"
    );
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap(),
        "identical configured runs must have byte-identical detailed histories"
    );
    assert!(first
        .history
        .iter()
        .all(|s| (quaternion_norm(s.attitude_wxyz) - 1.0).abs() <= 1e-12));
    assert!(first.history.iter().any(|s| s.phase == FlightPhase::Rail));
    assert!(first.history.iter().any(|s| s.phase == FlightPhase::Ascent));
    assert!(first
        .history
        .iter()
        .any(|s| s.phase == FlightPhase::Descent));
    assert_eq!(first.history.last().unwrap().phase, FlightPhase::Grounded);
}

#[test]
fn sixdof_hash_covers_vehicle_wind_and_launch_categories() {
    let (rocket, env) = alpha_iii();
    let motor = c6();
    let config = SimConfig::default();
    let hash = |engine: &SixDofEngine| {
        engine
            .run(&rocket, &motor, &env, &config)
            .unwrap()
            .input_hash
    };

    let base = configured_engine();
    let mut changed_vehicle = tree_vehicle();
    changed_vehicle.cp_from_nose_m += 0.001;
    let changed_vehicle = SixDofEngine::new(
        changed_vehicle,
        Wind3DProfile::calm(),
        SixDofLaunch::vertical(),
    )
    .unwrap();
    let changed_wind = engine_for(1.0, 0.0);
    let changed_launch = engine_for(0.0, 1.0_f64.to_radians());

    assert_ne!(hash(&base), hash(&changed_vehicle));
    assert_ne!(hash(&base), hash(&changed_wind));
    assert_ne!(hash(&base), hash(&changed_launch));
}

#[test]
fn two_in_plane_fixtures_match_planar_within_declared_residual_gates() {
    let (rocket, env) = alpha_iii();
    let motor = c6();
    let config = SimConfig::default();

    for (wind_ms, tilt_deg) in [(3.0_f64, 0.0_f64), (5.0_f64, 0.0_f64)] {
        let tilt_rad = tilt_deg.to_radians();
        let planar = simulate_planar(
            &rocket,
            &motor,
            &env,
            &planar_vehicle(tilt_rad),
            &WindProfile::constant(wind_ms),
            &config,
        );
        let six = engine_for(wind_ms, tilt_rad)
            .run_detailed(&rocket, &motor, &env, &config)
            .unwrap();
        let apogee_relative = (six.summary.apogee_m - planar.apogee_m).abs() / planar.apogee_m;
        let range_absolute = (six.landing_position_m[0] - planar.landing_range_m).abs();
        let range_relative = range_absolute / planar.landing_range_m.abs().max(1e-12);
        let pitch_absolute = (six.weathercock_pitch_deg - planar.weathercock_deg).abs();

        assert!(apogee_relative <= 0.02, "apogee residual {apogee_relative}");
        assert!(
            range_absolute <= 5.0 && range_relative <= 0.05,
            "range residual: {range_absolute} m, {range_relative:e} relative"
        );
        assert!(pitch_absolute <= 2.0, "pitch residual {pitch_absolute} deg");
    }
}

#[test]
fn tilted_rail_history_is_finite_bounded_and_roll_free() {
    let (rocket, env) = alpha_iii();
    let result = engine_for(2.0, 5.0_f64.to_radians())
        .run_detailed(&rocket, &c6(), &env, &SimConfig::default())
        .unwrap();

    assert!(result.summary.apogee_m.is_finite() && result.summary.apogee_m > 0.0);
    assert!(result.summary.landing_time_s > result.summary.apogee_time_s);
    for sample in &result.history {
        assert!(sample.time_s.is_finite());
        assert!(sample.position_m.iter().all(|v| v.is_finite()));
        assert!(sample.velocity_ms.iter().all(|v| v.is_finite()));
        assert!(sample.attitude_wxyz.iter().all(|v| v.is_finite()));
        assert!(sample.angular_rate_rad_s.iter().all(|v| v.is_finite()));
        assert!((quaternion_norm(sample.attitude_wxyz) - 1.0).abs() <= 1e-12);
        assert!(sample.angular_rate_rad_s[2].abs() <= 1e-10);
        if matches!(sample.phase, FlightPhase::Ascent | FlightPhase::Descent) {
            let tilt = body_axis_tilt_deg(sample.attitude_wxyz);
            assert!((0.0..=45.0).contains(&tilt), "attitude tilt {tilt} deg");
        }
    }
}

#[test]
fn invalid_configured_parameters_are_rejected_before_integration() {
    let mut invalid = tree_vehicle();
    invalid.pitch_yaw_inertia_kgm2 = f64::NAN;
    assert!(
        SixDofEngine::new(invalid, Wind3DProfile::calm(), SixDofLaunch::vertical())
            .err()
            .unwrap()
            .contains("inertia")
    );

    let engine = configured_engine();
    let (rocket, env) = alpha_iii();
    assert!(engine
        .run_detailed(
            &rocket,
            &c6(),
            &env,
            &SimConfig {
                dt_s: 0.0,
                max_time_s: 10.0,
            },
        )
        .unwrap_err()
        .contains("timestep"));
}
