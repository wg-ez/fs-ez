use super::*;

#[test]
fn tokens_are_32_bytes_base64url() -> Result<(), AppError> {
    let first = random_token()?;
    let second = random_token()?;
    assert_ne!(first, second);
    assert_eq!(first.len(), TOKEN_LEN);
    assert!(is_valid_token_format(&first));
    assert_eq!(
        URL_SAFE_NO_PAD.decode(&first).map(|b| b.len()),
        Ok(TOKEN_BYTES)
    );
    Ok(())
}

#[test]
fn format_rejects_short_bad_and_padded_tokens() {
    assert!(!is_valid_token_format(""));
    assert!(!is_valid_token_format(&"a".repeat(42)));
    assert!(!is_valid_token_format(&"a".repeat(44)));
    assert!(!is_valid_token_format(&format!("{}=", "a".repeat(43))));
    assert!(!is_valid_token_format(&"$".repeat(43)));
}

#[test]
fn hash_is_lowercase_hex_64() {
    let hash = hash_token("token");
    assert_eq!(hash.len(), 64);
    assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert!(hash.bytes().all(|byte| !byte.is_ascii_uppercase()));
    assert_eq!(
        hash,
        "3c469e9d6c5875d37a43f353d4f88e61fcf812c66eee3457465a40b0da4153e0"
    );
}
