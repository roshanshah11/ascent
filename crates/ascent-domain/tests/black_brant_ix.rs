//! Step 1 acceptance (evidence half): the checked-in Black Brant IX reference
//! mission is fully evidence-graded and structurally a two-stage vehicle.
//!
//! Honesty contract (see `data/reference/nasa-black-brant-ix/SOURCES.md`):
//! every physical input is bound to an authoritative, derived, or approximate
//! claim. The checked-in excerpt and original NASA PDF have distinct hashes;
//! authoritative grades apply only to values transcribed directly from NASA.
//! This is a reference scenario, not a historical-flight reconstruction.

use ascent_domain::reference::{black_brant_ix_reference, BLACK_BRANT_IX_MISSION_ID};

fn repo_root() -> String {
    format!("{}/../..", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn black_brant_ix_is_fully_evidence_graded() {
    let reference = black_brant_ix_reference().expect("reference mission loads");

    assert_eq!(reference.mission_id, BLACK_BRANT_IX_MISSION_ID);
    assert_eq!(reference.mission_id, "nasa.black-brant-ix.reference");

    // Structurally a two-stage vehicle: a single StageCoupler splits the
    // stack into booster + sustainer.
    assert_eq!(reference.vehicle.stages().len(), 2);
    reference.vehicle.validate().expect("vehicle tree is valid");

    // Every evidence binding carries a non-empty field path and a grade in
    // the closed set.
    assert!(!reference.evidence.is_empty());
    for binding in &reference.evidence {
        assert!(!binding.field_path.is_empty(), "empty field path");
        assert!(
            matches!(
                binding.grade.as_str(),
                "authoritative" | "derived" | "approximate"
            ),
            "binding {} has out-of-contract grade {}",
            binding.field_path,
            binding.grade
        );
        // A binding must resolve to a declared source.
        assert!(
            reference.sources.iter().any(|s| s.id == binding.source_id),
            "binding {} references unknown source {}",
            binding.field_path,
            binding.source_id
        );
    }

    // Every source digest is 64 lowercase hex characters — the real SHA-256
    // of a checked-in artifact, never a placeholder or empty-file digest.
    assert!(!reference.sources.is_empty());
    for source in &reference.sources {
        assert_eq!(
            source.sha256.len(),
            64,
            "source {} digest length",
            source.id
        );
        assert!(
            source
                .sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
            "source {} digest must be lowercase hex",
            source.id
        );
        assert!(
            matches!(
                source.grade.as_str(),
                "authoritative" | "derived" | "approximate"
            ),
            "source {} grade {}",
            source.id,
            source.grade
        );
        assert!(
            source.rights.contains("U.S. Government work"),
            "source {} must record its redistribution basis",
            source.id
        );
    }
}

#[test]
fn every_bound_field_has_exactly_one_binding() {
    let reference = black_brant_ix_reference().unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for binding in &reference.evidence {
        assert!(
            seen.insert(binding.field_path.clone()),
            "field {} is bound more than once",
            binding.field_path
        );
    }
}

#[test]
fn derived_bindings_carry_a_formula_and_inputs_approximate_carry_impact() {
    let reference = black_brant_ix_reference().unwrap();
    for binding in &reference.evidence {
        match binding.grade.as_str() {
            "derived" => {
                assert!(
                    binding.formula.as_deref().is_some_and(|f| !f.is_empty()),
                    "derived binding {} needs a formula",
                    binding.field_path
                );
                assert!(
                    !binding.inputs.is_empty(),
                    "derived binding {} needs named inputs",
                    binding.field_path
                );
            }
            "approximate" => {
                assert!(
                    binding.impact.as_deref().is_some_and(|i| !i.is_empty()),
                    "approximate binding {} needs an impact statement",
                    binding.field_path
                );
            }
            // Authoritative bindings map a value directly from the cited
            // source, so they carry neither a derivation formula nor an
            // approximation impact statement.
            "authoritative" => {}
            other => panic!("unexpected grade {other}"),
        }
    }
}

#[test]
fn mission_ships_the_non_historical_caveat() {
    let reference = black_brant_ix_reference().unwrap();
    assert!(
        reference
            .caveat
            .contains("not a reconstruction of a historical NASA flight"),
        "caveat was: {}",
        reference.caveat
    );
}

#[test]
fn checked_in_source_excerpt_and_original_pdf_have_distinct_hashes() {
    let reference = black_brant_ix_reference().unwrap();
    reference.verify(Some(&repo_root())).unwrap();
    let source = reference.sources.first().unwrap();
    assert_eq!(
        source.original_sha256.as_deref(),
        Some("2420152937522d053fcb78813ba944181882f7e5314919d47e692610fbe90495")
    );
    assert_ne!(
        source.original_sha256.as_deref(),
        Some(source.sha256.as_str())
    );
}

#[test]
fn reference_does_not_use_hobby_motors() {
    let reference = black_brant_ix_reference().unwrap();
    let names: Vec<_> = reference
        .motors
        .iter()
        .map(|motor| motor.designation.as_str())
        .collect();
    assert_eq!(names, ["TERRIER-MK70-REF", "BLACK-BRANT-V-REF"]);
}

#[test]
fn reference_motors_are_two_and_validate() {
    let reference = black_brant_ix_reference().unwrap();
    assert_eq!(reference.motors.len(), 2);
    for motor in &reference.motors {
        motor.validate().expect("reference motor validates");
    }
}
