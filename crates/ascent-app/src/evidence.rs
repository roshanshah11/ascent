//! Evidence / provenance report (Day 8): everything a skeptic needs to
//! audit a run — input hash, model identities, data provenance, explicit
//! assumptions, and a live timestep-convergence check.

use ascent_sim::{convergence_report, ConvergenceReport, SimConfig};
use serde::{Deserialize, Serialize};

use crate::design::{build_flight, Design};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotorEvidence {
    pub designation: String,
    pub manufacturer: String,
    /// Provenance block carried verbatim from the bundled motor JSON
    /// (certification body, ThrustCurve.org simfile id, etc.).
    pub provenance: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceReport {
    /// SHA-256 of the canonical sim input — matches the run footer.
    pub input_hash: String,
    pub engine: String,
    pub models: Vec<String>,
    pub assumptions: Vec<String>,
    pub motor: MotorEvidence,
    pub convergence: ConvergenceReport,
    /// External validation summary (Day 3, docs/EVIDENCE.md).
    pub validation: String,
}

pub fn evidence_for(design: &Design) -> Result<EvidenceReport, String> {
    let (rocket, motor, env) = build_flight(design)?;
    let config = SimConfig::default();
    let convergence = convergence_report(&rocket, &motor, &env, &config);
    let result = ascent_sim::simulate_vertical(&rocket, &motor, &env, &config);
    let summary = ascent_sim::SimSummary::from_result(&result, &rocket, &motor, &env, &config);

    Ok(EvidenceReport {
        input_hash: summary.input_hash,
        engine: format!("ascent-sim {} · fixed-step RK4, dt {} s", env!("CARGO_PKG_VERSION"), config.dt_s),
        models: vec![
            "US Standard Atmosphere 1976 (validated vs published density table)".into(),
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_matches_run_hash_and_converges() {
        let design = Design::reference();
        let ev = evidence_for(&design).unwrap();
        let record = crate::design::run_design(&design).unwrap();
        assert_eq!(ev.input_hash, record.summary.input_hash, "evidence must describe the same run");
        assert!(ev.convergence.converged, "reference design must be timestep-converged");
        assert!(!ev.motor.provenance.is_null(), "motor provenance must be carried through");
    }

    #[test]
    fn evidence_rejects_invalid_design() {
        let mut d = Design::reference();
        d.motor_designation = "Z99".into();
        assert!(evidence_for(&d).is_err());
    }
}
