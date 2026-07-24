//! Evidence / provenance report (Day 8): everything a skeptic needs to
//! audit a run — input hash, model identities, data provenance, explicit
//! assumptions, and a live timestep-convergence check.

use ascent_sim::{convergence_report, ConvergenceReport, NativeEngine, SimConfig, SimEngine};
use serde::{Deserialize, Serialize};

use crate::credibility::{scorecard, Scorecard, ScorecardInputs};
use crate::design::{build_flight, Design};
use crate::study::{study_input_hash, Study, StudyId};
use ascent_domain::evidence::EvidenceLevel;
use ascent_domain::vehicle::{PartKind, Vehicle};
use ascent_review::{
    structural::{evaluate_structural, FinMaterial, StructuralCheck, StructuralInputs},
    StructuralConfig,
};
use ascent_sim::AtmosphereProfile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotorEvidence {
    pub designation: String,
    pub manufacturer: String,
    /// Provenance block carried verbatim from the bundled motor JSON
    /// (certification body, ThrustCurve.org simfile id, etc.).
    pub provenance: serde_json::Value,
}

/// Which solver produced the numbers — sourced from the SimEngine trait,
/// never hardcoded, so bridge engines report themselves correctly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineEvidence {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceReport {
    /// SHA-256 of the canonical sim input — matches the run footer.
    pub input_hash: String,
    pub engine: EngineEvidence,
    pub models: Vec<String>,
    pub assumptions: Vec<String>,
    pub motor: MotorEvidence,
    pub convergence: ConvergenceReport,
    /// External validation summary (Day 3, docs/EVIDENCE.md).
    pub validation: String,
    /// Named evidence ladder, preserving intended use and caveats instead of
    /// collapsing professional validation into one pass/fail badge.
    pub validation_cases: Vec<ValidationCaseSummary>,
    /// NASA-7009-inspired factor scores + per-quantity regime flags
    /// (v0.2 Step 7, contract in docs/CREDIBILITY.md).
    pub credibility: Scorecard,
    /// Present when evidence was requested for a specific completed study.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub study: Option<StudyEvidence>,
    /// Explicit hash relations make it impossible to silently imply that a
    /// flat-flight evidence hash and a tree-backed study hash are identical.
    #[serde(default)]
    pub links: Vec<HashLink>,
    /// Structural flight-readiness margins are included whenever evidence is
    /// read against a tree-backed study, carrying each formula's source.
    #[serde(default)]
    pub structural_checks: Vec<StructuralCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCaseSummary {
    pub case_id: String,
    pub title: String,
    pub evidence_level: EvidenceLevel,
    pub intended_use: String,
    pub validity_domain: Vec<String>,
    pub caveats: Vec<String>,
    pub known_mismatches: Vec<String>,
    pub case_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyEvidence {
    pub id: StudyId,
    pub result_input_hash: String,
    pub current_input_hash: String,
    pub is_stale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashLink {
    pub relation: String,
    pub from_hash: String,
    pub to_hash: String,
}

pub fn evidence_for(design: &Design) -> Result<EvidenceReport, String> {
    evidence_for_with_atmosphere(design, None)
}

pub fn evidence_for_with_atmosphere(
    design: &Design,
    atmosphere: Option<&AtmosphereProfile>,
) -> Result<EvidenceReport, String> {
    let (rocket, motor, mut env) = build_flight(design)?;
    if let Some(model) = atmosphere.and_then(AtmosphereProfile::atmosphere_model) {
        env.atmosphere = model;
    }
    let config = SimConfig::default();
    let convergence = convergence_report(&rocket, &motor, &env, &config);
    let engine: &dyn SimEngine = &NativeEngine;
    let summary = engine.run(&rocket, &motor, &env, &config)?;

    let credibility = scorecard(&ScorecardInputs {
        max_velocity_ms: summary.max_velocity_ms,
        converged: convergence.converged,
        motor_user_imported: motor.provenance.to_string().contains("user-imported"),
    });

    Ok(EvidenceReport {
        input_hash: summary.input_hash,
        engine: EngineEvidence {
            id: engine.id().into(),
            version: engine.version().into(),
        },
        models: vec![
            format!("Fixed-step RK4 integrator, dt {} s", config.dt_s),
            atmosphere.map_or_else(
                || "US Standard Atmosphere 1976 (verified against published 1976 density table)".into(),
                |profile| format!("Imported atmosphere profile: {} ({} layers)", profile.name, profile.layers.len()),
            ),
            "Point-mass vertical flight: pad hold, rail phase, quadratic drag".into(),
            "Recovery: parachute deployed at apogee, quadratic chute drag".into(),
            "Motor: RASP-convention thrust interpolation, impulse-proportional mass depletion".into(),
        ],
        assumptions: vec![
            "Vertical launch, no wind, no weathercocking".into(),
            format!("Constant drag coefficient Cd = {} (no Mach dependence)", design.cd),
            "Standard gravity 9.80665 m/s² (no latitude correction)".into(),
            "Chute deploys exactly at apogee (no ejection-charge delay model)".into(),
        ],
        motor: MotorEvidence {
            designation: motor.designation.clone(),
            manufacturer: motor.manufacturer.clone(),
            provenance: motor.provenance.clone(),
        },
        convergence,
        validation: "Matched-config apogee within 1.4% and max velocity within 0.1% of an OpenRocket 24.12 export (docs/EVIDENCE.md)".into(),
        validation_cases: validation_case_summaries()?,
        credibility,
        study: None,
        links: vec![],
        structural_checks: vec![],
    })
}

fn validation_case_summaries() -> Result<Vec<ValidationCaseSummary>, String> {
    [
        include_bytes!("../../../data/validation/cases/analytic-constant-thrust.json").as_slice(),
        include_bytes!("../../../data/validation/cases/openrocket-24_12.json").as_slice(),
        include_bytes!("../../../data/validation/cases/rocketpy-cross-validation.json").as_slice(),
        include_bytes!("../../../data/validation/cases/valkyrie-2025-flight.json").as_slice(),
    ]
    .into_iter()
    .map(|bytes| {
        let case = ascent_review::validation::ValidationCase::from_canonical_bytes(bytes)?;
        let case_hash = case.case_hash()?;
        Ok(ValidationCaseSummary {
            case_id: case.case_id,
            title: case.title,
            evidence_level: case.evidence_level,
            intended_use: case.intended_use,
            validity_domain: case.validity_domain,
            caveats: case.caveats,
            known_mismatches: case.known_mismatches,
            case_hash,
        })
    })
    .collect()
}

/// Evidence plus an explicit provenance link for one study result. The study
/// hash includes the vehicle tree (and any imported atmosphere); the base
/// evidence hash covers the flat flight configuration, so they are related
/// but intentionally not conflated.
pub fn evidence_for_study(
    vehicle: &Vehicle,
    design: &Design,
    atmosphere: Option<&AtmosphereProfile>,
    study: &Study,
) -> Result<EvidenceReport, String> {
    let result_input_hash = study
        .results
        .as_ref()
        .ok_or_else(|| format!("study {} has not been run", study.id.0))?
        .input_hash
        .clone();
    let current_input_hash = study_input_hash(vehicle, design, atmosphere, study);
    let mut report = evidence_for(design)?;
    report.study = Some(StudyEvidence {
        id: study.id,
        is_stale: result_input_hash != current_input_hash,
        result_input_hash: result_input_hash.clone(),
        current_input_hash: current_input_hash.clone(),
    });
    report.links.push(HashLink {
        relation: "study_result_to_current_input".into(),
        from_hash: result_input_hash,
        to_hash: current_input_hash,
    });
    let (aero, _) = ascent_aero::Vehicle::from_tree(vehicle)?;
    let (rocket, motor, env) = build_flight(design)?;
    let sim = ascent_sim::simulate_vertical(&rocket, &motor, &env, &SimConfig::default());
    let config = StructuralConfig::default();
    report.structural_checks = evaluate_structural(&StructuralInputs {
        fin_span_m: aero.fins.span_m,
        fin_root_chord_m: aero.fins.root_chord_m,
        fin_thickness_m: tree_fin_thickness_m(vehicle)?,
        fin_material: FinMaterial {
            name: config.fin_material.name,
            elastic_modulus_pa: config.fin_material.elastic_modulus_pa,
        },
        air_density_kg_m3: 1.225,
        predicted_max_velocity_ms: sim.max_velocity_ms,
        max_motor_thrust_n: motor
            .thrust_curve
            .iter()
            .map(|(_, thrust)| *thrust)
            .fold(0.0, f64::max),
        airframe_thrust_rating_n: config.airframe_thrust_rating_n,
        rail_exit_velocity_ms: sim.rail_exit_velocity_ms,
    });
    Ok(report)
}

fn tree_fin_thickness_m(vehicle: &Vehicle) -> Result<f64, String> {
    for root in &vehicle.parts {
        for child in &root.children {
            if let PartKind::FinSet { thickness_mm, .. } = child.kind {
                if thickness_mm > 0.0 {
                    return Ok(thickness_mm / 1000.0);
                }
            }
        }
    }
    Err("vehicle needs a fin set with positive thickness".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_matches_run_hash_and_converges() {
        let design = Design::reference();
        let ev = evidence_for(&design).unwrap();
        let record = crate::design::run_design(&design).unwrap();
        assert_eq!(
            ev.input_hash, record.summary.input_hash,
            "evidence must describe the same run"
        );
        assert!(
            ev.convergence.converged,
            "reference design must be timestep-converged"
        );
        assert!(
            !ev.motor.provenance.is_null(),
            "motor provenance must be carried through"
        );
    }

    #[test]
    fn evidence_engine_comes_from_the_trait_not_a_hardcoded_string() {
        let ev = evidence_for(&Design::reference()).unwrap();
        let native = NativeEngine;
        assert_eq!(ev.engine.id, native.id());
        assert_eq!(ev.engine.version, native.version());
        assert!(!ev.engine.version.is_empty());
    }

    #[test]
    fn evidence_carries_a_full_scorecard_for_the_reference_design() {
        use crate::credibility::Regime;
        let ev = evidence_for(&Design::reference()).unwrap();
        assert_eq!(
            ev.credibility.factors.len(),
            5,
            "docs/CREDIBILITY.md fixes five factors"
        );
        assert!(ev.credibility.factors.iter().all(|f| !f.basis.is_empty()));
        // The reference bird is subsonic and converged: every quantity is within
        // its evidence-backed reference regime (Regime::Validated, defined in
        // docs/CREDIBILITY.md — not a claim of real-flight validation).
        assert!(ev
            .credibility
            .quantities
            .iter()
            .all(|q| q.regime == Regime::Validated));
        assert_eq!(ev.validation_cases.len(), 4);
        assert!(ev.validation_cases.iter().any(|case| {
            case.evidence_level == EvidenceLevel::FlightDataAvailable && !case.caveats.is_empty()
        }));
    }

    #[test]
    fn evidence_rejects_invalid_design() {
        let mut d = Design::reference();
        d.motor_designation = "Z99".into();
        assert!(evidence_for(&d).is_err());
    }

    #[test]
    fn study_evidence_keeps_result_and_current_hashes_explicitly_linked() {
        use crate::{Command, Document, StudyId, StudyKind};
        let mut doc = Document::default();
        doc.dispatch(Command::CreateStudy {
            name: "evidence".into(),
            kind: StudyKind::Dispersion { flights: 3 },
            engine: "native".into(),
            seed: 7,
        })
        .unwrap();
        crate::run_study_now(&mut doc, StudyId(1)).unwrap();
        let evidence = evidence_for_study(
            &doc.vehicle,
            &doc.design,
            doc.atmosphere.as_ref(),
            &doc.studies[0],
        )
        .unwrap();
        assert_eq!(evidence.study.as_ref().unwrap().id, StudyId(1));
        assert_eq!(evidence.links[0].relation, "study_result_to_current_input");
        assert_eq!(evidence.links[0].from_hash.len(), 64);
        assert_eq!(evidence.structural_checks.len(), 3);
        assert!(evidence
            .structural_checks
            .iter()
            .all(|check| !check.source.is_empty()));
    }
}
