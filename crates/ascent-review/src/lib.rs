//! Flight Review (Day 7): evaluate a full design against the cited rule
//! pack, and repair infeasible configurations deterministically.
//!
//! A ReviewDesign couples the parametric vehicle (stability, from
//! ascent-aero) with the flight configuration (drag, chute, rail — flown by
//! ascent-sim). The solver never mutates physics: it searches over motor
//! choice and nose-ballast mass, re-running the same deterministic sim.

use ascent_aero::{
    stability_calibers_at, stability_pct_of_length_at, FlightQuantities, NoseShape, PointMass,
    RulePack, Vehicle,
};
use ascent_domain::Motor;
use ascent_sim::{
    simulate_vertical, AtmosphereModel, DragModel, Environment, Recovery, Rocket, SimConfig,
    SimResult,
};
use serde::{Deserialize, Serialize};

pub const BALLAST_NAME: &str = "solver ballast";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChuteConfig {
    pub cd: f64,
    pub area_m2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewDesign {
    pub vehicle: Vehicle,
    pub nose_shape: NoseShape,
    pub cd: f64,
    pub chute: Option<ChuteConfig>,
    pub rail_length_m: f64,
    pub motor_designation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub apogee_m: f64,
    pub rail_exit_velocity_ms: f64,
    pub quantities: FlightQuantities,
    pub checks: Vec<ascent_aero::CheckResult>,
    pub feasible: bool,
}

fn to_rocket(design: &ReviewDesign) -> Rocket {
    let r = design.vehicle.diameter_m() / 2.0;
    Rocket {
        name: design.vehicle.name.clone(),
        dry_mass_kg: design.vehicle.dry_mass_kg(),
        drag: Some(DragModel {
            cd: design.cd,
            reference_area_m2: std::f64::consts::PI * r * r,
        }),
        recovery: design.chute.as_ref().map(|c| Recovery {
            chute_cd: c.cd,
            chute_area_m2: c.area_m2,
        }),
    }
}

fn fly(design: &ReviewDesign, motor: &Motor) -> SimResult {
    let env = Environment {
        gravity_ms2: 9.80665,
        atmosphere: AtmosphereModel::Standard,
        rail_length_m: design.rail_length_m,
    };
    simulate_vertical(&to_rocket(design), motor, &env, &SimConfig::default())
}

/// Measure every rule-bound quantity. Stability is sampled across the burn
/// (CG moves monotonically for a rear motor, but sampling is cheap and makes
/// no monotonicity assumption).
fn measure(design: &ReviewDesign, motor: &Motor, result: &SimResult) -> FlightQuantities {
    let v = &design.vehicle;
    let burn = motor.burn_time();
    const SAMPLES: usize = 32;
    let mut min_cal = f64::INFINITY;
    let mut min_pct = f64::INFINITY;
    let mut max_pct = f64::NEG_INFINITY;
    for i in 0..=SAMPLES {
        let t = burn * i as f64 / SAMPLES as f64;
        min_cal = min_cal.min(stability_calibers_at(v, design.nose_shape, motor, t));
        let pct = stability_pct_of_length_at(v, design.nose_shape, motor, t);
        min_pct = min_pct.min(pct);
        max_pct = max_pct.max(pct);
    }
    FlightQuantities {
        rail_exit_velocity_ms: result.rail_exit_velocity_ms,
        min_stability_calibers: min_cal,
        fin_span_calibers: v.fins.span_m / v.diameter_m(),
        stability_pct_len_at_launch: stability_pct_of_length_at(v, design.nose_shape, motor, 0.0),
        stability_pct_len_min: min_pct,
        stability_pct_len_max: max_pct,
    }
}

pub fn evaluate(design: &ReviewDesign, motor: &Motor, rules: &RulePack) -> Review {
    let result = fly(design, motor);
    let quantities = measure(design, motor, &result);
    let checks = rules.check(&quantities);
    let feasible = checks.iter().all(|c| c.pass);
    Review {
        apogee_m: result.apogee_m,
        rail_exit_velocity_ms: result.rail_exit_velocity_ms,
        quantities,
        checks,
        feasible,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffEntry {
    pub field: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repair {
    pub design: ReviewDesign,
    pub review: Review,
    pub target_apogee_m: f64,
    pub achieved_apogee_m: f64,
    pub diff: Vec<DiffEntry>,
}

fn with_ballast(design: &ReviewDesign, ballast_kg: f64) -> ReviewDesign {
    let mut d = design.clone();
    d.vehicle.point_masses.retain(|p| p.name != BALLAST_NAME);
    if ballast_kg > 0.0 {
        // Ballast goes at the nose-cone base: helps CG, hurts apogee —
        // exactly the trade the solver exploits.
        d.vehicle.point_masses.push(PointMass {
            name: BALLAST_NAME.into(),
            mass_kg: ballast_kg,
            position_from_nose_m: d.vehicle.nose.length_m,
        });
    }
    d
}

fn existing_ballast_kg(design: &ReviewDesign) -> f64 {
    design
        .vehicle
        .point_masses
        .iter()
        .filter(|p| p.name == BALLAST_NAME)
        .map(|p| p.mass_kg)
        .sum()
}

/// Repair an infeasible design: for each candidate motor (in the caller's
/// order — deterministic), bisect nose ballast so apogee lands within
/// `tolerance_m` of `target_apogee_m`, then require every rule to pass.
/// Returns the first fully feasible repair, with a diff of every change.
pub fn solve(
    design: &ReviewDesign,
    motors: &[Motor],
    rules: &RulePack,
    target_apogee_m: f64,
    tolerance_m: f64,
) -> Result<Repair, String> {
    const MAX_BALLAST_KG: f64 = 2.0;
    const BISECT_ITERS: usize = 60;

    let mut reasons: Vec<String> = Vec::new();
    for motor in motors {
        let mut candidate = with_ballast(design, 0.0);
        candidate.motor_designation = motor.designation.clone();

        // Apogee decreases monotonically with ballast: bracket the target.
        let apogee_bare = fly(&candidate, motor).apogee_m;
        if apogee_bare + tolerance_m < target_apogee_m {
            reasons.push(format!(
                "{}: max apogee {:.0} m below target",
                motor.designation, apogee_bare
            ));
            continue;
        }

        let mut ballast = 0.0;
        if (apogee_bare - target_apogee_m).abs() > tolerance_m {
            let (mut lo, mut hi) = (0.0_f64, MAX_BALLAST_KG);
            let apogee_heavy = fly(&with_ballast(&candidate, hi), motor).apogee_m;
            if apogee_heavy > target_apogee_m + tolerance_m {
                reasons.push(format!(
                    "{}: even {} kg ballast leaves apogee {:.0} m above target",
                    motor.designation, MAX_BALLAST_KG, apogee_heavy
                ));
                continue;
            }
            for _ in 0..BISECT_ITERS {
                let mid = (lo + hi) / 2.0;
                let apogee = fly(&with_ballast(&candidate, mid), motor).apogee_m;
                if apogee > target_apogee_m {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            ballast = (lo + hi) / 2.0;
        }

        let repaired = with_ballast(&candidate, ballast);
        let review = evaluate(&repaired, motor, rules);
        let apogee_err = (review.apogee_m - target_apogee_m).abs();
        if apogee_err > tolerance_m {
            reasons.push(format!(
                "{}: could not settle within ±{} m of target",
                motor.designation, tolerance_m
            ));
            continue;
        }
        if !review.feasible {
            let failed: Vec<&str> = review
                .checks
                .iter()
                .filter(|c| !c.pass)
                .map(|c| c.rule_id.as_str())
                .collect();
            reasons.push(format!("{}: violates {}", motor.designation, failed.join(", ")));
            continue;
        }

        let mut diff = Vec::new();
        if design.motor_designation != motor.designation {
            diff.push(DiffEntry {
                field: "motor".into(),
                before: design.motor_designation.clone(),
                after: motor.designation.clone(),
            });
        }
        let before_ballast = existing_ballast_kg(design);
        if (before_ballast - ballast).abs() > 1e-6 {
            diff.push(DiffEntry {
                field: "nose ballast".into(),
                before: format!("{:.1} g", before_ballast * 1000.0),
                after: format!("{:.1} g", ballast * 1000.0),
            });
        }
        let achieved = review.apogee_m;
        return Ok(Repair {
            design: repaired,
            review,
            target_apogee_m,
            achieved_apogee_m: achieved,
            diff,
        });
    }
    Err(if reasons.is_empty() {
        "no candidate motors supplied".into()
    } else {
        format!("no feasible configuration found: {}", reasons.join("; "))
    })
}
