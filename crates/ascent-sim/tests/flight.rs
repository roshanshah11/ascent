use approx::assert_relative_eq;
use ascent_domain::Motor;
use ascent_sim::{
    convergence_report, input_hash, simulate_vertical, AtmosphereModel, DragModel, Environment,
    EventKind, Recovery, Rocket, SimConfig, SimSummary,
};

const C6_JSON: &str = include_str!("../../ascent-domain/data/motors/estes_c6.json");

fn c6() -> Motor {
    Motor::from_json(C6_JSON).unwrap()
}

/// Constant-thrust motor with negligible propellant mass, so the analytic
/// constant-mass closed form applies: a = F/m - g, v = a·t, h = a·t²/2,
/// apogee = h_b + v_b²/(2g).
fn constant_thrust_motor(thrust_n: f64, burn_s: f64) -> Motor {
    Motor {
        designation: "CONST".into(),
        manufacturer: "test".into(),
        raw_header: None,
        total_mass_kg: 1e-9,
        propellant_mass_kg: 1e-12,
        expected_total_impulse_ns: thrust_n * burn_s,
        thrust_curve: vec![
            (1e-9, thrust_n),
            (burn_s, thrust_n),
            (burn_s + 1e-9, 0.0),
        ],
        provenance: serde_json::Value::Null,
        diameter_mm: 0.0,
        length_mm: 0.0,
        expected_burn_time_s: burn_s,
        expected_avg_thrust_n: thrust_n,
        expected_max_thrust_n: thrust_n,
    }
}

fn dragless_rocket(mass_kg: f64) -> Rocket {
    Rocket {
        name: "analytic".into(),
        dry_mass_kg: mass_kg,
        drag: None,
        recovery: None,
    }
}

fn vacuum_env() -> Environment {
    Environment {
        gravity_ms2: 9.80665,
        atmosphere: AtmosphereModel::ConstantDensity(0.0),
        rail_length_m: 0.9,
    }
}

/// Estes Alpha III: ~34 g structure, 25 mm diameter, declared Cd 0.60,
/// 30 cm chute. Reference rocket for the Day-3 OpenRocket comparison.
fn alpha_iii() -> Rocket {
    let d = 0.025_f64;
    let chute_d = 0.30_f64;
    Rocket {
        name: "Estes Alpha III".into(),
        dry_mass_kg: 0.0340,
        drag: Some(DragModel {
            cd: 0.60,
            reference_area_m2: std::f64::consts::PI * (d / 2.0) * (d / 2.0),
        }),
        recovery: Some(Recovery {
            chute_cd: 0.75,
            chute_area_m2: std::f64::consts::PI * (chute_d / 2.0) * (chute_d / 2.0),
        }),
    }
}

// ---- atmosphere ----

#[test]
fn standard_atmosphere_matches_published_table() {
    let atm = AtmosphereModel::Standard;
    // 1976 US Standard Atmosphere reference densities.
    assert_relative_eq!(atm.density_at(0.0), 1.225, max_relative = 1e-3);
    assert_relative_eq!(atm.density_at(1000.0), 1.112, max_relative = 1e-3);
    assert_relative_eq!(atm.density_at(5000.0), 0.7364, max_relative = 1e-3);
    assert_relative_eq!(atm.density_at(11000.0), 0.3639, max_relative = 1e-3);
}

#[test]
fn density_decreases_with_altitude() {
    let atm = AtmosphereModel::Standard;
    let mut prev = atm.density_at(0.0);
    for h in (500..=11_000).step_by(500) {
        let rho = atm.density_at(h as f64);
        assert!(rho < prev);
        prev = rho;
    }
}

// ---- analytic constant-thrust / no-drag fixture ----

#[test]
fn analytic_constant_thrust_no_drag_matches_closed_form() {
    let g = 9.80665;
    let (f, m, tb) = (20.0, 0.5, 2.0);
    let a = f / m - g;
    let v_b = a * tb;
    let h_b = a * tb * tb / 2.0;
    let apogee = h_b + v_b * v_b / (2.0 * g);
    let apogee_t = tb + v_b / g;

    let motor = constant_thrust_motor(f, tb);
    let rocket = dragless_rocket(m);
    let config = SimConfig {
        dt_s: 0.0001,
        max_time_s: 60.0,
    };
    let r = simulate_vertical(&rocket, &motor, &vacuum_env(), &config);

    assert_relative_eq!(r.burnout_velocity_ms, v_b, max_relative = 1e-4);
    assert_relative_eq!(r.burnout_altitude_m, h_b, max_relative = 1e-4);
    assert_relative_eq!(r.apogee_m, apogee, max_relative = 1e-4);
    assert_relative_eq!(r.apogee_time_s, apogee_t, max_relative = 1e-4);
    assert_relative_eq!(r.max_velocity_ms, v_b, max_relative = 1e-3);
}

// ---- events ----

#[test]
fn event_timeline_is_complete_and_ordered() {
    let r = simulate_vertical(
        &alpha_iii(),
        &c6(),
        &Environment::default(),
        &SimConfig::default(),
    );
    let kinds: Vec<EventKind> = r.events.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        vec![
            EventKind::Liftoff,
            EventKind::RailExit,
            EventKind::Burnout,
            EventKind::Apogee,
            EventKind::RecoveryDeploy,
            EventKind::Landing,
        ]
    );
    // Strictly non-decreasing times.
    for pair in r.events.windows(2) {
        assert!(pair[0].t <= pair[1].t);
    }
}

#[test]
fn rail_exit_is_at_rail_length() {
    let env = Environment::default();
    let r = simulate_vertical(&alpha_iii(), &c6(), &env, &SimConfig::default());
    let rail = r.event(EventKind::RailExit).unwrap();
    assert_relative_eq!(rail.altitude_m, env.rail_length_m, max_relative = 1e-6);
    assert!(rail.velocity_ms > 0.0);
    assert!(r.rail_exit_velocity_ms > 0.0);
}

#[test]
fn apogee_velocity_is_zero() {
    let r = simulate_vertical(
        &alpha_iii(),
        &c6(),
        &Environment::default(),
        &SimConfig::default(),
    );
    let apogee = r.event(EventKind::Apogee).unwrap();
    assert!(apogee.velocity_ms.abs() < 0.05);
}

#[test]
fn landing_is_at_ground_level() {
    let r = simulate_vertical(
        &alpha_iii(),
        &c6(),
        &Environment::default(),
        &SimConfig::default(),
    );
    let landing = r.event(EventKind::Landing).unwrap();
    assert!(landing.altitude_m.abs() < 1e-6);
    assert!(landing.velocity_ms < 0.0, "landing must be descending");
    assert!(landing.t > r.apogee_time_s);
}

#[test]
fn rocket_too_heavy_never_lifts_off() {
    let rocket = dragless_rocket(10.0);
    let r = simulate_vertical(&rocket, &c6(), &Environment::default(), &SimConfig::default());
    assert!(r.event(EventKind::Liftoff).is_none());
    assert!(r.apogee_m <= 1e-9);
}

// ---- recovery descent ----

#[test]
fn chute_descent_reaches_terminal_velocity() {
    // v_t = sqrt(2 m g / (rho cda_total)); compare against the descent rate
    // just before landing (near sea level, standard density).
    let rocket = alpha_iii();
    let motor = c6();
    let r = simulate_vertical(
        &rocket,
        &motor,
        &Environment::default(),
        &SimConfig::default(),
    );
    let m = rocket.dry_mass_kg + motor.total_mass_kg - motor.propellant_mass_kg;
    let rec = rocket.recovery.as_ref().unwrap();
    let body = rocket.drag.as_ref().unwrap();
    let cda = rec.chute_cd * rec.chute_area_m2 + body.cd * body.reference_area_m2;
    let rho0 = AtmosphereModel::Standard.density_at(0.0);
    let v_t = (2.0 * m * 9.80665 / (rho0 * cda)).sqrt();
    assert_relative_eq!(-r.landing_velocity_ms, v_t, max_relative = 0.02);
}

#[test]
fn chute_makes_landing_slower_and_later_than_ballistic() {
    let motor = c6();
    let env = Environment::default();
    let config = SimConfig::default();
    let with_chute = simulate_vertical(&alpha_iii(), &motor, &env, &config);
    let mut ballistic = alpha_iii();
    ballistic.recovery = None;
    let without = simulate_vertical(&ballistic, &motor, &env, &config);
    assert!(with_chute.landing_velocity_ms.abs() < without.landing_velocity_ms.abs());
    assert!(with_chute.landing_time_s > without.landing_time_s);
    assert!(without.event(EventKind::RecoveryDeploy).is_none());
}

// ---- reference rocket ----

#[test]
fn alpha_iii_on_c6_lands_in_plausible_band() {
    let r = simulate_vertical(
        &alpha_iii(),
        &c6(),
        &Environment::default(),
        &SimConfig::default(),
    );
    assert!(
        r.apogee_m > 150.0 && r.apogee_m < 450.0,
        "apogee {} m outside plausible band",
        r.apogee_m
    );
    assert!(r.max_velocity_ms > 30.0 && r.max_velocity_ms < 120.0);
    assert!(r.burnout_time_s > 1.8 && r.burnout_time_s < 1.9);
    assert!(r.apogee_time_s > r.burnout_time_s);
}

#[test]
fn drag_reduces_apogee() {
    let motor = c6();
    let env = Environment::default();
    let config = SimConfig::default();
    let with_drag = simulate_vertical(&alpha_iii(), &motor, &env, &config);
    let mut no_drag = alpha_iii();
    no_drag.drag = None;
    let without = simulate_vertical(&no_drag, &motor, &env, &config);
    assert!(with_drag.apogee_m < without.apogee_m);
}

// ---- convergence (the Day-2 exit gate) ----

#[test]
fn reference_flight_converges_at_half_step() {
    let report = convergence_report(
        &alpha_iii(),
        &c6(),
        &Environment::default(),
        &SimConfig::default(),
    );
    assert!(
        report.converged,
        "apogee moved {} m when halving dt (apogees {:?})",
        report.apogee_delta_m, report.apogee_m
    );
    // Error must shrink as the step shrinks — unless both deltas are already
    // at the numerical noise floor (< 1 cm on a ~360 m apogee), where
    // monotonicity is meaningless and tiny is the proof.
    const NOISE_FLOOR_M: f64 = 0.01;
    assert!(
        report.apogee_delta_fine_m <= report.apogee_delta_m
            || (report.apogee_delta_m < NOISE_FLOOR_M
                && report.apogee_delta_fine_m < NOISE_FLOOR_M),
        "error grew above noise floor: {} -> {}",
        report.apogee_delta_m,
        report.apogee_delta_fine_m
    );
}

// ---- determinism + summary ----

#[test]
fn summary_is_deterministic() {
    let rocket = alpha_iii();
    let motor = c6();
    let env = Environment::default();
    let config = SimConfig::default();
    let s1 = SimSummary::from_result(
        &simulate_vertical(&rocket, &motor, &env, &config),
        &rocket,
        &motor,
        &env,
        &config,
    );
    let s2 = SimSummary::from_result(
        &simulate_vertical(&rocket, &motor, &env, &config),
        &rocket,
        &motor,
        &env,
        &config,
    );
    assert_eq!(
        serde_json::to_string(&s1).unwrap(),
        serde_json::to_string(&s2).unwrap()
    );
    assert_eq!(s1.input_hash, s2.input_hash);
    assert_eq!(s1.input_hash.len(), 64);
}

#[test]
fn input_hash_changes_when_inputs_change() {
    let motor = c6();
    let env = Environment::default();
    let config = SimConfig::default();
    let h1 = input_hash(&alpha_iii(), &motor, &env, &config);
    let mut heavier = alpha_iii();
    heavier.dry_mass_kg += 0.010;
    let h2 = input_hash(&heavier, &motor, &env, &config);
    assert_ne!(h1, h2);
}
