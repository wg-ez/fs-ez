use super::*;

#[test]
fn token_hash_validation_accepts_only_hex_64() {
    assert!(validate_token_hash(&"a".repeat(64)).is_ok());
    assert!(validate_token_hash(&"A1".repeat(32)).is_ok());
    assert!(validate_token_hash("").is_err());
    assert!(validate_token_hash(&"a".repeat(63)).is_err());
    assert!(validate_token_hash(&"g".repeat(64)).is_err());
    assert!(validate_token_hash(&"é".repeat(64)).is_err());
}

#[test]
fn user_username_is_trimmed_and_bounded() {
    assert_eq!(validate_username("  bob ").expect("nom valide"), "bob");
    assert!(validate_username("   ").is_err());
    assert!(validate_username(&"x".repeat(65)).is_err());
}

#[test]
fn token_name_is_trimmed_and_bounded() {
    assert_eq!(validate_token_name("  ci ").expect("nom valide"), "ci");
    assert!(validate_token_name("").is_err());
    assert!(validate_token_name(&"x".repeat(101)).is_err());
}
