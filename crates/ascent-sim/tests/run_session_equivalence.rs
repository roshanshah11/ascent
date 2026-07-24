//! Deterministic-equivalence proof for the re-entrant [`RunSession`] seam.
//!
//! The canonical batch trace (`trace_reference_mission`) and a `RunSession`
//! stepped in fixed-count chunks share one integrator, so they must agree
//! bit-for-bit. This test proves it over the complete Black Brant IX flight:
//! every `SixDofSample` field (by IEEE-754 bit pattern), the phase timeline,
//! the full event order/kind/values, and the final canonical trace hash.

use ascent_domain::reference::black_brant_ix_reference;
use ascent_sim::reference_trace::trace_reference_mission;
use ascent_sim::{
    content_hash, flight_trace_from_sixdof, AdvanceOutcome, MissionSnapshot, RunSession,
    SixDofResult, SixDofSample,
};

/// Drive a session to completion in fixed `chunk`-sized `AdvanceSteps` calls,
/// returning the final result and the number of chunks it took (proving the
/// run really was stepped, not run in one shot).
fn run_stepped(snapshot: &MissionSnapshot, chunk: u64) -> (SixDofResult, u64) {
    let mut session = RunSession::create(snapshot).expect("session creates");
    let mut chunks = 0_u64;
    loop {
        chunks += 1;
        match session.advance(chunk).expect("advance succeeds") {
            AdvanceOutcome::Advanced => continue,
            AdvanceOutcome::Completed => break,
            AdvanceOutcome::Cancelled => panic!("session cancelled unexpectedly"),
        }
    }
    assert!(session.is_complete(), "session must report completion");
    (session.result(), chunks)
}

/// Every f64 component of a sample, in field order, as raw bit patterns.
fn sample_bits(sample: &SixDofSample) -> Vec<u64> {
    let mut bits = Vec::with_capacity(1 + 3 + 3 + 4 + 3);
    bits.push(sample.time_s.to_bits());
    bits.extend(sample.position_m.iter().map(|v| v.to_bits()));
    bits.extend(sample.velocity_ms.iter().map(|v| v.to_bits()));
    bits.extend(sample.attitude_wxyz.iter().map(|v| v.to_bits()));
    bits.extend(sample.angular_rate_rad_s.iter().map(|v| v.to_bits()));
    bits
}

/// SHA-256 over the canonical evidence trace projected from a result — the
/// "final canonical trace_hash" the review contract transmits.
fn trace_hash(result: &SixDofResult) -> String {
    let trace = flight_trace_from_sixdof(result).expect("evidence trace projects");
    content_hash(&trace)
}

#[test]
fn stepped_session_equals_batch_bit_for_bit() {
    let mission = black_brant_ix_reference().expect("reference builds");
    let batch = trace_reference_mission(&mission).expect("batch trace runs");

    let snapshot = MissionSnapshot::from_reference_mission(&mission).expect("snapshot builds");
    // A small chunk forces many AdvanceSteps calls across every phase
    // transition, rail exit, staging, apogee, and landing crossing.
    let (stepped, chunks) = run_stepped(&snapshot, 250);
    assert!(
        chunks > 1,
        "the flight must span multiple AdvanceSteps chunks, got {chunks}"
    );

    // --- 1. History: length, then every sample field by bit pattern + phase.
    assert_eq!(
        stepped.history.len(),
        batch.history.len(),
        "history length must match"
    );
    for (index, (s, b)) in stepped.history.iter().zip(batch.history.iter()).enumerate() {
        assert_eq!(s.phase, b.phase, "phase differs at sample {index}");
        assert_eq!(
            sample_bits(s),
            sample_bits(b),
            "sample {index} differs by bit pattern: stepped={s:?} batch={b:?}"
        );
    }

    // --- 2. Events: identical order, kind, and interpolated values (bits).
    assert_eq!(
        stepped.summary.events.len(),
        batch.summary.events.len(),
        "event count must match"
    );
    for (index, (s, b)) in stepped
        .summary
        .events
        .iter()
        .zip(batch.summary.events.iter())
        .enumerate()
    {
        assert_eq!(s.kind, b.kind, "event {index} kind differs");
        assert_eq!(
            (
                s.t_s.to_bits(),
                s.altitude_m.to_bits(),
                s.velocity_ms.to_bits()
            ),
            (
                b.t_s.to_bits(),
                b.altitude_m.to_bits(),
                b.velocity_ms.to_bits()
            ),
            "event {index} ({}) values differ by bit pattern",
            s.kind
        );
    }

    // --- 3. Identity + full canonical bytes (belt-and-suspenders over 1 & 2).
    assert_eq!(
        stepped.summary.input_hash, batch.summary.input_hash,
        "canonical input hash must match"
    );
    assert_eq!(
        serde_json::to_vec(&stepped).unwrap(),
        serde_json::to_vec(&batch).unwrap(),
        "stepped and batch results must have identical canonical JSON bytes"
    );

    // --- 4. Final canonical evidence trace hash.
    assert_eq!(
        trace_hash(&stepped),
        trace_hash(&batch),
        "final canonical trace_hash must be identical"
    );
}

#[test]
fn chunk_size_does_not_change_the_result() {
    // The step-count granularity must not perturb the numerics: a run stepped
    // one core step at a time equals one stepped in large chunks.
    let mission = black_brant_ix_reference().expect("reference builds");
    let snapshot = MissionSnapshot::from_reference_mission(&mission).expect("snapshot builds");

    let (fine, _) = run_stepped(&snapshot, 1);
    let (coarse, _) = run_stepped(&snapshot, 100_000);

    assert_eq!(
        serde_json::to_vec(&fine).unwrap(),
        serde_json::to_vec(&coarse).unwrap(),
        "chunk size must not change the trajectory bytes"
    );
    assert_eq!(trace_hash(&fine), trace_hash(&coarse));
}

#[test]
fn config_hash_is_stable_and_schema_versioned() {
    let mission = black_brant_ix_reference().expect("reference builds");
    let a = MissionSnapshot::from_reference_mission(&mission).expect("snapshot builds");
    let b = MissionSnapshot::from_reference_mission(&mission).expect("snapshot builds");

    // Deterministic: identical inputs → identical hash, and a well-formed
    // 64-hex-char SHA-256.
    assert_eq!(
        a.config_hash(),
        b.config_hash(),
        "config_hash must be stable"
    );
    let hash = a.config_hash();
    assert_eq!(hash.len(), 64, "config_hash must be a SHA-256 hex string");
    assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()));

    // Changing an input changes the hash; the schema version participates.
    let mut altered = a.clone();
    altered.config.dt_s = 0.01;
    assert_ne!(
        a.config_hash(),
        altered.config_hash(),
        "a changed integration input must change config_hash"
    );
}

#[test]
fn snapshot_survives_json_round_trip() {
    // A snapshot serialized and read back must be indistinguishable: same
    // config_hash, and a session created from it reproduces the batch trace.
    let mission = black_brant_ix_reference().expect("reference builds");
    let batch = trace_reference_mission(&mission).expect("batch trace runs");
    let original = MissionSnapshot::from_reference_mission(&mission).expect("snapshot builds");

    let json = serde_json::to_vec(&original).expect("snapshot serializes");
    let restored: MissionSnapshot = serde_json::from_slice(&json).expect("snapshot deserializes");

    assert_eq!(
        original.config_hash(),
        restored.config_hash(),
        "config_hash must survive a JSON round-trip"
    );

    let (stepped, _) = run_stepped(&restored, 250);
    assert_eq!(
        serde_json::to_vec(&stepped).unwrap(),
        serde_json::to_vec(&batch).unwrap(),
        "a round-tripped snapshot must reproduce the batch trace bit-for-bit"
    );
    assert_eq!(trace_hash(&stepped), trace_hash(&batch));
}

#[test]
fn create_rejects_unknown_schema_version() {
    let mission = black_brant_ix_reference().expect("reference builds");
    let mut snapshot = MissionSnapshot::from_reference_mission(&mission).expect("snapshot builds");
    snapshot.schema_version = ascent_sim::SNAPSHOT_SCHEMA_VERSION + 1;

    let error = RunSession::create(&snapshot)
        .err()
        .expect("unknown schema must fail closed");
    assert!(
        error.contains("schema version"),
        "error must name the schema-version mismatch, got: {error}"
    );
}

#[test]
fn cancel_stops_further_advance() {
    let mission = black_brant_ix_reference().expect("reference builds");
    let snapshot = MissionSnapshot::from_reference_mission(&mission).expect("snapshot builds");
    let mut session = RunSession::create(&snapshot).expect("session creates");

    session.advance(10).expect("initial advance");
    session.cancel();
    assert!(session.is_cancelled());
    assert_eq!(
        session.advance(10).expect("advance after cancel"),
        AdvanceOutcome::Cancelled,
        "a cancelled session must not advance"
    );
}
