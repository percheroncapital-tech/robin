// Asserts the canonical envelope JSON shape never loses/renames a v1 field.
// If this test must change to REMOVE or RENAME a field, that is a breaking
// change and requires a major version bump + CONVENTIONS.md update — not allowed silently.
use fleet_log::{validate, envelope::ENVELOPE_VERSION};

#[test]
fn v1_required_keys_are_frozen() {
    assert_eq!(ENVELOPE_VERSION, 1);
    let v1 = r#"{"v":1,"ts":"t","level":"INFO","service":"s","tenant":"t","target":"x","msg":"m"}"#;
    assert!(validate(v1).is_ok(), "a v1 line must validate forever (additive-only)");
}
