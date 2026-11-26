use super::*;

#[test]
fn core_error_becomes_invalid() {
    let error: StoreError = CoreError::InvalidName("x".to_string()).into();
    assert!(matches!(error, StoreError::Invalid(ref message) if message.contains("x")));
    assert_eq!(error.to_string(), "invalid: invalid name: x");
}
