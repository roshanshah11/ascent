use ascent_domain::Motor;
use ascent_sim::{simulate_vertical, DragModel, Environment, Rocket, SimConfig, SimSummary};

const C6_JSON: &str = include_str!("../../../ascent-domain/data/motors/estes_c6.json");

fn main() {
    let motor = Motor::from_json(C6_JSON).expect("bundled C6 motor must validate");
    let d = 0.025_f64;
    let rocket = Rocket {
        name: "Estes Alpha III".into(),
        dry_mass_kg: 0.0340,
        drag: Some(DragModel {
            cd: 0.60,
            reference_area_m2: std::f64::consts::PI * (d / 2.0) * (d / 2.0),
        }),
    };
    let env = Environment::default();
    let config = SimConfig::default();

    let result = simulate_vertical(&rocket, &motor, &env, &config);
    let summary = SimSummary::from_result(&result, &rocket, &motor, &env, &config);
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
}
