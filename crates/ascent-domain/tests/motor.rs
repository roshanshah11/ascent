use approx::assert_relative_eq;
use ascent_domain::{Motor, MotorError};

const C6_JSON: &str = include_str!("../data/motors/estes_c6.json");

fn c6() -> Motor {
    Motor::from_json(C6_JSON).expect("bundled C6 must load and validate")
}

fn simple_motor() -> Motor {
    // Constant 10 N for 2 s, ramping instantaneously per RASP implicit start:
    // curve (0.001, 10) -> (2.0, 10) -> (2.001, 0). Impulse ~= 20 N·s.
    Motor {
        designation: "TEST".into(),
        manufacturer: "test".into(),
        total_mass_kg: 0.100,
        propellant_mass_kg: 0.050,
        expected_total_impulse_ns: 20.0,
        thrust_curve: vec![(0.001, 10.0), (2.0, 10.0), (2.001, 0.0)],
        provenance: serde_json::Value::Null,
        diameter_mm: 0.0,
        length_mm: 0.0,
        expected_burn_time_s: 2.001,
        expected_avg_thrust_n: 10.0,
        expected_max_thrust_n: 10.0,
    }
}

// ---- curve validation ----

#[test]
fn bundled_c6_validates() {
    let m = c6();
    assert_eq!(m.designation, "C6");
    assert_relative_eq!(m.propellant_mass_kg, 0.0108, epsilon = 1e-9);
}

#[test]
fn rejects_empty_curve() {
    let mut m = simple_motor();
    m.thrust_curve.clear();
    assert_eq!(m.validate(), Err(MotorError::EmptyCurve));
}

#[test]
fn rejects_non_monotonic_time() {
    let mut m = simple_motor();
    m.thrust_curve = vec![(0.5, 10.0), (0.5, 10.0), (2.0, 0.0)];
    assert_eq!(m.validate(), Err(MotorError::NonMonotonicTime(1)));
}

#[test]
fn rejects_negative_thrust() {
    let mut m = simple_motor();
    m.thrust_curve = vec![(0.5, 10.0), (1.0, -1.0), (2.0, 0.0)];
    assert_eq!(m.validate(), Err(MotorError::NegativeThrust(1)));
}

#[test]
fn rejects_nonzero_final_thrust() {
    let mut m = simple_motor();
    m.thrust_curve = vec![(0.5, 10.0), (2.0, 10.0)];
    m.expected_total_impulse_ns = 17.5;
    assert_eq!(m.validate(), Err(MotorError::NonZeroFinalThrust(10.0)));
}

#[test]
fn rejects_zero_or_negative_start_time() {
    let mut m = simple_motor();
    m.thrust_curve = vec![(0.0, 0.0), (1.0, 10.0), (2.0, 0.0)];
    assert_eq!(m.validate(), Err(MotorError::NonPositiveStartTime));
}

#[test]
fn rejects_propellant_exceeding_total_mass() {
    let mut m = simple_motor();
    m.propellant_mass_kg = 0.200;
    assert_eq!(
        m.validate(),
        Err(MotorError::PropellantExceedsTotal {
            prop: 0.200,
            total: 0.100
        })
    );
}

#[test]
fn rejects_impulse_mismatch() {
    let mut m = simple_motor();
    m.expected_total_impulse_ns = 40.0; // computed is ~20
    assert!(matches!(
        m.validate(),
        Err(MotorError::ImpulseMismatch { .. })
    ));
}

// ---- impulse ----

#[test]
fn simple_motor_impulse_is_trapezoid_exact() {
    // Implicit (0,0) -> (0.001,10): 0.005 N·s
    // (0.001,10) -> (2.0,10): 19.99 N·s
    // (2.0,10) -> (2.001,0): 0.005 N·s
    let m = simple_motor();
    assert_relative_eq!(m.total_impulse(), 20.0, epsilon = 1e-9);
}

#[test]
fn c6_impulse_close_to_certified() {
    // NAR-certified total impulse is 8.82 N·s; trapezoid over the cert
    // samples must land within 5%.
    let m = c6();
    let imp = m.total_impulse();
    assert!(
        (imp - 8.82).abs() / 8.82 < 0.05,
        "impulse {imp} too far from 8.82"
    );
}

// ---- interpolation boundaries ----

#[test]
fn thrust_before_zero_is_zero() {
    assert_eq!(simple_motor().thrust_at(-0.5), 0.0);
}

#[test]
fn thrust_at_zero_is_zero_implicit_start() {
    assert_eq!(simple_motor().thrust_at(0.0), 0.0);
}

#[test]
fn thrust_ramps_from_implicit_start() {
    // halfway between (0,0) and (0.001,10) -> 5 N
    assert_relative_eq!(simple_motor().thrust_at(0.0005), 5.0, epsilon = 1e-9);
}

#[test]
fn thrust_at_sample_point_is_exact() {
    let m = c6();
    assert_relative_eq!(m.thrust_at(0.192), 14.09, epsilon = 1e-9);
}

#[test]
fn thrust_interpolates_between_samples() {
    // C6: (0.031, 0.946) -> (0.092, 4.826); midpoint t=0.0615 -> 2.886
    let m = c6();
    assert_relative_eq!(m.thrust_at(0.0615), (0.946 + 4.826) / 2.0, epsilon = 1e-9);
}

#[test]
fn thrust_at_burnout_is_zero() {
    let m = c6();
    assert_eq!(m.thrust_at(m.burn_time()), 0.0);
}

#[test]
fn thrust_after_burnout_is_zero() {
    let m = c6();
    assert_eq!(m.thrust_at(m.burn_time() + 1.0), 0.0);
}

#[test]
fn burn_time_is_last_sample() {
    assert_relative_eq!(c6().burn_time(), 1.86, epsilon = 1e-9);
}

// ---- mass depletion ----

#[test]
fn no_propellant_consumed_before_ignition() {
    let m = c6();
    assert_eq!(m.propellant_consumed_at(0.0), 0.0);
    assert_eq!(m.mass_at(0.0), m.total_mass_kg);
}

#[test]
fn all_propellant_consumed_at_burnout() {
    let m = c6();
    assert_relative_eq!(
        m.propellant_consumed_at(m.burn_time()),
        m.propellant_mass_kg,
        epsilon = 1e-12
    );
}

#[test]
fn all_propellant_consumed_after_burnout() {
    let m = c6();
    assert_relative_eq!(
        m.propellant_consumed_at(m.burn_time() + 5.0),
        m.propellant_mass_kg,
        epsilon = 1e-12
    );
    assert_relative_eq!(
        m.mass_at(m.burn_time() + 5.0),
        m.total_mass_kg - m.propellant_mass_kg,
        epsilon = 1e-12
    );
}

#[test]
fn mass_depletion_is_monotonic_nonincreasing() {
    let m = c6();
    let mut prev = m.mass_at(0.0);
    let mut t = 0.0;
    while t <= m.burn_time() + 0.1 {
        let cur = m.mass_at(t);
        assert!(cur <= prev + 1e-12, "mass increased at t={t}");
        prev = cur;
        t += 0.01;
    }
}

#[test]
fn half_impulse_consumes_half_propellant() {
    // Depletion is proportional to impulse fraction; for the constant-thrust
    // simple motor, the time delivering half the impulse consumes half the
    // propellant.
    let m = simple_motor();
    // Find t where cumulative impulse = 10.0 (half of 20): with the tiny ramp,
    // 0.005 + 10*(t-0.001) = 10 -> t = 1.0005
    assert_relative_eq!(
        m.propellant_consumed_at(1.0005),
        m.propellant_mass_kg / 2.0,
        epsilon = 1e-9
    );
}
