use approx::assert_relative_eq;
use ascent_domain::Motor;
use ascent_sim::{
    input_hash, simulate_vertical, DragModel, Environment, Rocket, SimConfig, SimSummary,
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
    }
}

/// Estes Alpha III: ~34 g structure, 25 mm diameter, declared Cd 0.60.
/// Reference rocket for the Day-3 OpenRocket comparison.
fn alpha_iii() -> Rocket {
    let d = 0.025_f64;
    Rocket {
        name: "Estes Alpha III".into(),
        dry_mass_kg: 0.0340,
        drag: Some(DragModel {
            cd: 0.60,
            reference_area_m2: std::f64::consts::PI * (d / 2.0) * (d / 2.0),
        }),
    }
}

// ---- analytic constant-thrust / no-drag fixture ----

#[test]
fn analytic_constant_thrust_no_drag_matches_closed_form() {
    // F = 20 N, m = 0.5 kg, burn 2 s, g = 9.80665.
    let g = 9.80665;
    let (f, m, tb) = (20.0, 0.5, 2.0);
    let a = f / m - g; // 30.19335 m/s²
    let v_b = a * tb;
    let h_b = a * tb * tb / 2.0;
    let apogee = h_b + v_b * v_b / (2.0 * g);
    let apogee_t = tb + v_b / g;

    let motor = constant_thrust_motor(f, tb);
    let rocket = dragless_rocket(m);
    let env = Environment::default();
    let config = SimConfig {
        dt_s: 0.0001,
        max_time_s: 60.0,
    };
    let r = simulate_vertical(&rocket, &motor, &env, &config);

    assert_relative_eq!(r.burnout_velocity_ms, v_b, max_relative = 1e-4);
    assert_relative_eq!(r.burnout_altitude_m, h_b, max_relative = 1e-4);
    assert_relative_eq!(r.apogee_m, apogee, max_relative = 1e-4);
    assert_relative_eq!(r.apogee_time_s, apogee_t, max_relative = 1e-4);
    assert_relative_eq!(r.max_velocity_ms, v_b, max_relative = 1e-3);
}

#[test]
fn rk4_converges_with_smaller_timestep() {
    let motor = constant_thrust_motor(20.0, 2.0);
    let rocket = dragless_rocket(0.5);
    let env = Environment::default();
    let coarse = simulate_vertical(
        &rocket,
        &motor,
        &env,
        &SimConfig {
            dt_s: 0.01,
            max_time_s: 60.0,
        },
    );
    let fine = simulate_vertical(
        &rocket,
        &motor,
        &env,
        &SimConfig {
            dt_s: 0.0025,
            max_time_s: 60.0,
        },
    );
    // Apogee difference between dt and dt/4 must be small (convergence).
    // Bound is dominated by the first-order error at the thrust
    // discontinuity (burn start/stop), not RK4's O(dt^4) — Day 2's
    // event-aligned stepping tightens this.
    assert!(
        (coarse.apogee_m - fine.apogee_m).abs() < 0.25,
        "coarse {} vs fine {}",
        coarse.apogee_m,
        fine.apogee_m
    );
}

// ---- pad behavior ----

#[test]
fn rocket_too_heavy_never_lifts_off() {
    // 10 kg on a C6: thrust never exceeds weight.
    let rocket = dragless_rocket(10.0);
    let motor = c6();
    let env = Environment::default();
    let r = simulate_vertical(&rocket, &motor, &env, &SimConfig::default());
    assert!(r.liftoff_time_s.is_nan() || r.apogee_m <= 0.0);
    assert!(r.apogee_m <= 0.0 + 1e-9);
}

// ---- reference rocket with drag ----

#[test]
fn alpha_iii_on_c6_lands_in_plausible_band() {
    // Estes advertises ~1100 ft (335 m) max altitude for Alpha III on C6-5.
    // Vertical no-wind sim with declared constant Cd must land in a wide
    // physical band; exact agreement is Day 3's OpenRocket fixture job.
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
