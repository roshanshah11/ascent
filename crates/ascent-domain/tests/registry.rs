use ascent_domain::MotorRegistry;
use serde_json::json;

// This upstream cert file bakes the delay into the common_name token itself
// ("B6-0" rather than "B6" + delay "0"), so registering it adds a new
// designation rather than replacing the bundled "B6".
const B6_0: &str = include_str!("../../../data/eng-samples/estes_b6_cert.eng");
// Same designation ("C6") as the bundled stock motor, so registering it
// exercises the replace-on-duplicate path.
const C6: &str = include_str!("../../../data/eng-samples/estes_c6_cert.eng");
const D12_MISSING_ZERO: &str =
    include_str!("../../../data/eng-samples/estes_d12_missing_terminal_zero.eng");

#[test]
fn bundled_registry_has_the_three_stock_motors() {
    let reg = MotorRegistry::bundled();
    for designation in ["C6", "B6", "D12"] {
        assert!(reg.get(designation).is_some(), "missing bundled motor {designation}");
    }
    assert_eq!(reg.list().len(), 3);
}

#[test]
fn register_eng_adds_a_new_designation_alongside_the_bundled_set() {
    let mut reg = MotorRegistry::bundled();
    let outcome = reg
        .register_eng(B6_0, json!({"source": "conformance corpus"}))
        .expect("B6-0 should register");
    assert_eq!(outcome.first_designation, "B6-0");
    assert!(outcome.replaced.is_empty(), "a new designation replaces nothing");
    let m = reg.get("B6-0").expect("B6-0 must be findable after registration");
    assert_eq!(m.manufacturer, "E");
    // Bundled "B6" is untouched; the registry now also has "B6-0".
    assert!(reg.get("B6").is_some(), "bundled B6 must still be present");
    assert_eq!(reg.list().len(), 4, "B6-0 is a new designation, so the registry grows");
}

#[test]
fn register_eng_replaces_existing_designation_rather_than_duplicating() {
    let mut reg = MotorRegistry::bundled();
    let before = reg.get("C6").unwrap().provenance.clone();
    let outcome = reg.register_eng(C6, json!({"source": "replacement"})).unwrap();
    let after = reg.get("C6").unwrap().provenance.clone();
    assert_ne!(before, after, "provenance should reflect the newly registered source");
    assert_eq!(reg.list().len(), 3, "replace must not grow the registry");
    assert_eq!(
        outcome.replaced,
        vec!["C6".to_string()],
        "overwriting a designation must be reported, not silent"
    );
}

#[test]
fn register_eng_propagates_parse_errors_as_strings() {
    let mut reg = MotorRegistry::bundled();
    let err = reg
        .register_eng(D12_MISSING_ZERO, json!({}))
        .expect_err("malformed source must not register");
    assert!(err.contains("line"), "error string should carry the line number: {err}");
}

#[test]
fn get_unknown_designation_is_none() {
    let reg = MotorRegistry::bundled();
    assert!(reg.get("Z99").is_none());
}
