//! Hachage et vérification des mots de passe en Argon2id.

use std::sync::OnceLock;

use argon2::password_hash::phc::PasswordHash;
use argon2::{Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version};

use crate::error::AppError;

/// Longueur minimale du mot de passe utilisateur, en caractères.
pub const MIN_PASSWORD_LEN: usize = 12;
/// Coût mémoire Argon2 en Kio, aligné sur `wg-ez`.
pub const ARGON2_M_COST: u32 = 19_456;
/// Coût temporel Argon2.
pub const ARGON2_T_COST: u32 = 2;
/// Parallélisme Argon2, runtime mono-thread.
pub const ARGON2_P_COST: u32 = 1;

fn argon2() -> Result<Argon2<'static>, AppError> {
    let params = Params::new(ARGON2_M_COST, ARGON2_T_COST, ARGON2_P_COST, None)
        .map_err(|error| AppError::Invalid(format!("argon2 params: {error}")))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

fn hash_error(error: impl std::fmt::Display) -> AppError {
    AppError::Invalid(format!("password hash: {error}"))
}

/// Argon2id bloquant, sel aléatoire par hash, minimum 12 caractères.
pub fn hash_password(password: &str) -> Result<String, AppError> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(AppError::Invalid(format!(
            "password must contain at least {MIN_PASSWORD_LEN} characters"
        )));
    }
    argon2()?
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(hash_error)
}

/// `Ok(false)` si le mot de passe ne correspond pas, `Err` si le hash est illisible.
pub fn verify_password(hash: &str, password: &str) -> Result<bool, AppError> {
    let parsed = PasswordHash::new(hash).map_err(hash_error)?;
    Ok(argon2()?
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Hash factice calculé une seule fois par processus, même coût pour un utilisateur absent.
fn dummy_hash() -> Result<&'static str, AppError> {
    static DUMMY: OnceLock<String> = OnceLock::new();
    if let Some(cached) = DUMMY.get() {
        return Ok(cached.as_str());
    }
    let fresh = hash_password("dummy-password-that-never-matches-0000")?;
    Ok(DUMMY.get_or_init(|| fresh).as_str())
}

/// Même travail Argon2 quand l'utilisateur n'existe pas, retourne toujours `Ok(false)`.
pub fn verify_or_dummy(hash: Option<&str>, password: &str) -> Result<bool, AppError> {
    match hash {
        Some(hash) => verify_password(hash, password),
        None => {
            let dummy = dummy_hash()?;
            let _ = verify_password(dummy, password)?;
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests;
