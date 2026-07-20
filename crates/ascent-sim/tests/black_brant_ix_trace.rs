//! Deterministic two-stage trace + evidence repeatability for the Black Brant
//! IX reference mission (vertical-slice Step 1, sim half).

use ascent_domain::reference::{black_brant_ix_reference, BLACK_BRANT_IX_MISSION_ID};
use ascent_sim::reference_trace::{mission_to_sixdof_stages, trace_reference_mission};

/// Repo root: `stored_path` values in the manifest are repo-relative, and
/// `ReferenceMission::verify` joins them onto this root.
fn repo_root() -> String {
    // CARGO_MANIFEST_DIR is <repo>/crates/ascent-sim.
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by harness");
    std::path::Path::new(&crate_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root is two levels above the crate dir")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn reference_sources_validate_rights_and_hashes() {
    let mission = black_brant_ix_reference().expect("reference builds");
    // Every source must declare rights and its stored artifact's SHA-256 must
    // match the recorded digest (recomputed from the checked-in file).
    mission
        .verify(Some(&repo_root()))
        .expect("sources validate");
}

#[test]
fn mission_id_matches_constant() {
    let mission = black_brant_ix_reference().expect("reference builds");
    assert_eq!(mission.mission_id, BLACK_BRANT_IX_MISSION_ID);
    assert_eq!(mission.mission_id, "nasa.black-brant-ix.reference");
}

#[test]
fn repeated_complete_traces_are_byte_identical() {
    let mission = black_brant_ix_reference().expect("reference builds");

    let first = trace_reference_mission(&mission).expect("first trace runs");
    let second = trace_reference_mission(&mission).expect("second trace runs");

    // Identity alone is insufficient: every event and history sample must be
    // byte-identical for the Unity playback contract.
    assert_eq!(
        first.summary.input_hash, second.summary.input_hash,
        "repeated traces must have identical identity"
    );
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap(),
        "repeated complete traces must have identical canonical JSON bytes"
    );
}

#[test]
fn trace_spans_launch_to_landing() {
    let mission = black_brant_ix_reference().expect("reference builds");
    let result = trace_reference_mission(&mission).expect("trace runs");

    let kinds: Vec<&str> = result
        .summary
        .events
        .iter()
        .map(|e| e.kind.as_str())
        .collect();
    assert_eq!(
        kinds,
        [
            "Liftoff",
            "RailExit",
            "Burnout",
            "StageSeparation",
            "StageIgnition",
            "Burnout",
            "Apogee",
            "RecoveryDeploy",
            "Landing",
        ],
        "trace must preserve the complete two-stage launch-to-landing order; final sample = {:?}",
        result.history.last()
    );
    assert!(
        result
            .summary
            .events
            .windows(2)
            .all(|pair| pair[0].t_s <= pair[1].t_s),
        "event times must be monotonic"
    );

    // Forward progress: apogee above the rail, landing back at the ground.
    assert!(
        result.summary.apogee_m > 1.0,
        "apogee must be a plausible positive altitude, got {}",
        result.summary.apogee_m
    );
    assert!(
        result.summary.landing_velocity_ms.is_finite(),
        "landing velocity must be finite"
    );
}

#[test]
fn stages_are_mapped_in_physical_burn_order() {
    let mission = black_brant_ix_reference().expect("reference builds");
    let stages = mission_to_sixdof_stages(&mission).expect("stage mapping succeeds");
    let designations: Vec<_> = stages
        .iter()
        .map(|stage| stage.motor.designation.as_str())
        .collect();
    assert_eq!(designations, ["TERRIER-MK70-REF", "BLACK-BRANT-V-REF"]);
}
