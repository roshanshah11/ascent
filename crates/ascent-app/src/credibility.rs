//! NASA-STD-7009-inspired credibility scorecard (v0.2 Step 7).
//!
//! Contract lives in docs/CREDIBILITY.md: five factors in a fixed order,
//! integer scores 0–4, every score carrying a `basis` string that cites a
//! checked-in file or fixture, and a per-quantity Validated/Extrapolated
//! flag where every Extrapolated quantity names its reason. This is an
//! evidence-and-limits summary, never a certification.

use serde::{Deserialize, Serialize};

/// Sea-level speed of sound, US Standard Atmosphere 1976. Used only to
/// convert max velocity into a Mach number for regime flagging — a
/// conservative choice, since the true speed of sound drops with altitude.
pub const SPEED_OF_SOUND_SEA_LEVEL_MS: f64 = 340.294;

/// Above this Mach the constant-Cd, incompressible model leaves its
/// evidence-backed regime (docs/EVIDENCE.md covers subsonic flight only).
pub const SUBSONIC_EVIDENCE_BOUNDARY_MACH: f64 = 0.8;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Factor {
    pub name: String,
    /// Integer 0–4 per the rubric table in docs/CREDIBILITY.md.
    pub score: u8,
    /// Human-readable evidence citation — always names a checked-in file.
    pub basis: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "reason", rename_all = "snake_case")]
pub enum Regime {
    Validated,
    /// Outside the evidence-backed regime; the String names why.
    Extrapolated(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuantityFlag {
    pub quantity: String,
    pub regime: Regime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scorecard {
    /// Always the five factors of docs/CREDIBILITY.md, in its fixed order.
    pub factors: Vec<Factor>,
    pub quantities: Vec<QuantityFlag>,
}

/// Everything the scorecard needs, extracted from a run so the scoring
/// itself is a pure, deterministic function.
#[derive(Debug, Clone)]
pub struct ScorecardInputs {
    pub max_velocity_ms: f64,
    pub converged: bool,
    /// True when the motor came from a user-supplied `.eng` file rather
    /// than the bundled, certification-cited stock set.
    pub motor_user_imported: bool,
}

pub fn scorecard(inputs: &ScorecardInputs) -> Scorecard {
    let mach = inputs.max_velocity_ms / SPEED_OF_SOUND_SEA_LEVEL_MS;
    let subsonic = mach < SUBSONIC_EVIDENCE_BOUNDARY_MACH;

    let verification = if inputs.converged {
        Factor {
            name: "Verification".into(),
            score: 3,
            basis: "Golden regression pin and timestep-convergence checks are checked in; \
                    see crates/ascent-app/tests/regression.rs and crates/ascent-sim (convergence_report tests)."
                .into(),
        }
    } else {
        Factor {
            name: "Verification".into(),
            score: 1,
            basis: "This configuration failed the live dt/2 convergence check \
                    (crates/ascent-sim convergence_report); the checked-in regression pin \
                    (crates/ascent-app/tests/regression.rs) does not cover it."
                .into(),
        }
    };

    let validation = if subsonic {
        Factor {
            name: "Validation".into(),
            score: 3,
            basis: "Matched-config comparison against a frozen OpenRocket 24.12 export \
                    (apogee within 1.4%, max velocity within 0.1%); see docs/EVIDENCE.md and \
                    crates/ascent-sim/tests/fixtures/openrocket_alpha3_c6.json."
                .into(),
        }
    } else {
        Factor {
            name: "Validation".into(),
            score: 1,
            basis: format!(
                "The only external comparison (docs/EVIDENCE.md, \
                 crates/ascent-sim/tests/fixtures/openrocket_alpha3_c6.json) is subsonic; \
                 predicted max Mach {mach:.2} is outside what it can validate."
            ),
        }
    };

    let pedigree = if inputs.motor_user_imported {
        Factor {
            name: "Input Pedigree".into(),
            score: 2,
            basis: "Motor curve is a user-supplied RASP .eng file parsed per docs/RASP_FORMAT.md; \
                    the file's own origin and certification are unverified."
                .into(),
        }
    } else {
        Factor {
            name: "Input Pedigree".into(),
            score: 3,
            basis: "Bundled motor with certification provenance JSON carried verbatim \
                    (data/motors/, validated against data/eng-samples/MANIFEST.json)."
                .into(),
        }
    };

    let uncertainty = Factor {
        name: "Uncertainty".into(),
        score: 2,
        basis: "Known residuals and model limits are recorded in docs/EVIDENCE.md and seeded \
                Monte Carlo dispersion exists (crates/ascent-sim/src/dispersion.rs), but \
                quantified uncertainty is not yet attached to every reported quantity."
            .into(),
    };

    let regime = if subsonic {
        Factor {
            name: "Regime Applicability".into(),
            score: 3,
            basis: format!(
                "Predicted max Mach {mach:.2} is inside the subsonic evidence regime \
                 documented in docs/EVIDENCE.md (boundary Mach {SUBSONIC_EVIDENCE_BOUNDARY_MACH})."
            ),
        }
    } else {
        Factor {
            name: "Regime Applicability".into(),
            score: 1,
            basis: format!(
                "Predicted max Mach {mach:.2} exceeds the subsonic evidence boundary \
                 (Mach {SUBSONIC_EVIDENCE_BOUNDARY_MACH}); docs/EVIDENCE.md records no \
                 transonic or supersonic comparison."
            ),
        }
    };

    let flag_for = |quantity: &str| QuantityFlag {
        quantity: quantity.into(),
        regime: if subsonic {
            Regime::Validated
        } else {
            Regime::Extrapolated(format!(
                "predicted max Mach {mach:.2} is past the subsonic evidence boundary \
                 (Mach {SUBSONIC_EVIDENCE_BOUNDARY_MACH}); the constant-Cd incompressible \
                 model and the docs/EVIDENCE.md comparison cover subsonic flight only"
            ))
        },
    };

    Scorecard {
        factors: vec![verification, validation, pedigree, uncertainty, regime],
        quantities: vec![flag_for("apogee_m"), flag_for("max_velocity_ms")],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_inputs() -> ScorecardInputs {
        ScorecardInputs {
            max_velocity_ms: 65.0, // reference bird is comfortably subsonic
            converged: true,
            motor_user_imported: false,
        }
    }

    #[test]
    fn factors_come_in_the_contract_order_with_bounded_scores() {
        let card = scorecard(&reference_inputs());
        let names: Vec<&str> = card.factors.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Verification",
                "Validation",
                "Input Pedigree",
                "Uncertainty",
                "Regime Applicability"
            ],
            "docs/CREDIBILITY.md fixes the factor order"
        );
        for f in &card.factors {
            assert!(f.score <= 4, "{} score out of range: {}", f.name, f.score);
        }
    }

    #[test]
    fn every_basis_cites_a_checked_in_file() {
        // House rule: no unexplained numbers. Each basis must point a reader
        // at a real file or fixture in the repo.
        for inputs in [
            reference_inputs(),
            ScorecardInputs {
                max_velocity_ms: 500.0,
                converged: false,
                motor_user_imported: true,
            },
        ] {
            for f in scorecard(&inputs).factors {
                assert!(
                    ["docs/", "crates/", "data/"].iter().any(|p| f.basis.contains(p)),
                    "{} basis cites no checked-in file: {}",
                    f.name,
                    f.basis
                );
            }
        }
    }

    #[test]
    fn reference_scores_are_deterministic_and_fully_validated() {
        let a = scorecard(&reference_inputs());
        let b = scorecard(&reference_inputs());
        assert_eq!(a, b, "identical inputs must give identical scorecards");
        for q in &a.quantities {
            assert_eq!(q.regime, Regime::Validated, "{} should be validated", q.quantity);
        }
    }

    #[test]
    fn supersonic_flips_apogee_to_extrapolated_with_a_named_reason() {
        let card = scorecard(&ScorecardInputs {
            max_velocity_ms: 500.0, // Mach ~1.47
            converged: true,
            motor_user_imported: false,
        });
        let apogee = card
            .quantities
            .iter()
            .find(|q| q.quantity == "apogee_m")
            .unwrap();
        match &apogee.regime {
            Regime::Extrapolated(reason) => {
                assert!(reason.contains("Mach"), "reason must name the boundary: {reason}");
                assert!(
                    reason.contains("docs/EVIDENCE.md"),
                    "reason must cite the evidence file: {reason}"
                );
            }
            Regime::Validated => panic!("supersonic apogee must be Extrapolated"),
        }
        let regime_factor = card.factors.last().unwrap();
        assert!(regime_factor.score < 3, "regime factor must drop outside the boundary");
    }

    #[test]
    fn user_imported_motor_lowers_input_pedigree() {
        let mut inputs = reference_inputs();
        inputs.motor_user_imported = true;
        let card = scorecard(&inputs);
        let pedigree = &card.factors[2];
        assert_eq!(pedigree.name, "Input Pedigree");
        assert_eq!(pedigree.score, 2);
        assert!(pedigree.basis.contains("RASP_FORMAT"));
    }

    #[test]
    fn transonic_boundary_is_exact() {
        let just_under = scorecard(&ScorecardInputs {
            max_velocity_ms: SPEED_OF_SOUND_SEA_LEVEL_MS * 0.79,
            ..reference_inputs()
        });
        let at_boundary = scorecard(&ScorecardInputs {
            max_velocity_ms: SPEED_OF_SOUND_SEA_LEVEL_MS * 0.80,
            ..reference_inputs()
        });
        assert!(just_under.quantities.iter().all(|q| q.regime == Regime::Validated));
        assert!(at_boundary
            .quantities
            .iter()
            .all(|q| matches!(q.regime, Regime::Extrapolated(_))));
    }
}
