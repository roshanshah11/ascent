use ascent_domain::Motor;
use ascent_sim::{
    flight_trace_from_dispersion, flight_trace_from_vertical, simulate_vertical, AtmosphereModel,
    CompactRun, DispersionSummary, DragModel, Environment, LandingEllipse, Rocket, SimConfig,
};

fn c6() -> Motor {
    Motor::from_json(include_str!(
        "../../ascent-domain/data/motors/estes_c6.json"
    ))
    .unwrap()
}

#[test]
fn dispersion_summary_adapts_without_changing_golden_bytes() {
    let summary = DispersionSummary {
        seed: 42,
        samples: 2,
        vary: vec![],
        apogee_p5_m: 99.0,
        apogee_p50_m: 100.0,
        apogee_p95_m: 101.0,
        landing_mean_m: 5.0,
        landing_ellipse: LandingEllipse {
            a_m: 2.0,
            b_m: 1.0,
            bearing_deg: 0.0,
        },
        runs: vec![
            CompactRun {
                apogee_m: 99.0,
                landing_range_m: 4.0,
                max_aoa_deg: 2.0,
            },
            CompactRun {
                apogee_m: 101.0,
                landing_range_m: 6.0,
                max_aoa_deg: 3.0,
            },
        ],
    };
    let before = serde_json::to_vec(&summary).unwrap();
    let trace = flight_trace_from_dispersion(&summary, &"22".repeat(32)).unwrap();
    trace.validate().unwrap();
    assert_eq!(before, serde_json::to_vec(&summary).unwrap());
    assert_eq!(trace.channels[0].samples.len(), 2);
}

#[test]
fn vertical_result_adapts_to_valid_trace_without_mutating_numerical_output() {
    let rocket = Rocket {
        name: "trace fixture".into(),
        dry_mass_kg: 0.034,
        drag: Some(DragModel {
            cd: 0.6,
            reference_area_m2: 0.000_490_873_9,
        }),
        recovery: None,
    };
    let result = simulate_vertical(
        &rocket,
        &c6(),
        &Environment {
            gravity_ms2: 9.80665,
            atmosphere: AtmosphereModel::Standard,
            rail_length_m: 0.9,
        },
        &SimConfig::default(),
    );
    let before = serde_json::to_vec(&result).unwrap();
    let trace = flight_trace_from_vertical(&result, &"11".repeat(32)).unwrap();

    trace.validate().unwrap();
    assert_eq!(before, serde_json::to_vec(&result).unwrap());
    assert!(trace
        .channels
        .iter()
        .any(|channel| channel.id == "truth.position"));
    assert!(trace
        .channels
        .iter()
        .any(|channel| channel.id == "truth.velocity"));
    assert_eq!(trace.events.len(), result.events.len());
}
