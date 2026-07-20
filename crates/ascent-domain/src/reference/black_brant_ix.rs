//! Evidence-graded NASA Black Brant IX class reference mission.

use serde::Deserialize;

use crate::motor::Motor;
use crate::reference::{
    EvidenceBinding, MissionCaveat, MissionId, ReferenceError, ReferenceMission, Source,
};
use crate::vehicle::Vehicle;

pub const BLACK_BRANT_IX_MISSION_ID: &str = "nasa.black-brant-ix.reference";

const REQUIRED_EVIDENCE_FIELDS: &[&str] = &[
    "vehicle.stage_architecture",
    "vehicle.terrier_geometry",
    "vehicle.black_brant_geometry",
    "vehicle.payload_envelope",
    "motor.black_brant_published_performance",
    "motor.black_brant_reference_curve",
    "vehicle.metric_conversions",
    "motor.terrier_performance",
    "vehicle.dry_masses_and_fin_geometry",
    "simulation.aerodynamics_and_recovery",
];

#[derive(Deserialize)]
struct Manifest {
    mission_id: String,
    caveat: String,
    sources: Vec<Source>,
    evidence: Vec<EvidenceBinding>,
}

#[derive(Deserialize)]
struct VehicleFixture {
    mission_id: String,
    vehicle: Vehicle,
    motors: Vec<Motor>,
}

/// Load the checked-in reference data. Engineering values live in reviewable
/// JSON rather than being disguised as Rust constants.
pub fn black_brant_ix_reference() -> Result<ReferenceMission, ReferenceError> {
    let manifest: Manifest = serde_json::from_str(include_str!(
        "../../../../data/reference/nasa-black-brant-ix/manifest.json"
    ))
    .map_err(|source| ReferenceError::Parse {
        artifact: "Black Brant IX manifest",
        source,
    })?;
    let fixture: VehicleFixture = serde_json::from_str(include_str!(
        "../../../../data/reference/nasa-black-brant-ix/vehicle.json"
    ))
    .map_err(|source| ReferenceError::Parse {
        artifact: "Black Brant IX vehicle fixture",
        source,
    })?;

    if manifest.mission_id != fixture.mission_id || manifest.mission_id != BLACK_BRANT_IX_MISSION_ID
    {
        return Err(ReferenceError::Validation(
            "Black Brant IX manifest and vehicle mission ids differ".into(),
        ));
    }

    let mission = ReferenceMission {
        mission_id: MissionId(manifest.mission_id),
        vehicle: fixture.vehicle,
        evidence: manifest.evidence,
        sources: manifest.sources,
        motors: fixture.motors,
        caveat: MissionCaveat(manifest.caveat),
    };
    mission.verify(None).map_err(ReferenceError::Validation)?;
    mission
        .require_evidence_fields(REQUIRED_EVIDENCE_FIELDS)
        .map_err(ReferenceError::Validation)?;
    Ok(mission)
}
