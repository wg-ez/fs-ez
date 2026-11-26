use super::*;

#[test]
fn key_and_value_are_bounded() {
    assert!(validate_key("theme").is_ok());
    assert!(validate_key("").is_err());
    assert!(validate_key("  ").is_err());
    assert!(validate_key(&"k".repeat(101)).is_err());
    assert!(validate_value(&"v".repeat(4096)).is_ok());
    assert!(validate_value(&"v".repeat(4097)).is_err());
}
