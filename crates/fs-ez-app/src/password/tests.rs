use super::*;

#[test]
fn policy_rejects_short_passwords() {
    assert!(hash_password("short").is_err());
    assert!(hash_password("twelve-chars").is_ok());
}

#[test]
fn hash_and_verify_round_trip() -> Result<(), AppError> {
    let hash = hash_password("a-very-long-password")?;
    assert!(verify_password(&hash, "a-very-long-password")?);
    assert!(!verify_password(&hash, "a-very-long-passwore")?);
    assert!(hash.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
    Ok(())
}

#[test]
fn unknown_user_pays_argon2_but_fails() -> Result<(), AppError> {
    assert!(!verify_or_dummy(None, "whatever-password")?);
    let hash = hash_password("correct-horse-pwrd")?;
    assert!(verify_or_dummy(Some(&hash), "correct-horse-pwrd")?);
    assert!(!verify_or_dummy(Some(&hash), "wrong-password12")?);
    Ok(())
}

#[test]
fn invalid_hash_is_an_error() {
    assert!(verify_password("not-a-phc-hash", "whatever-password").is_err());
}
