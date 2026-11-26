//! Génération et empreinte des jetons opaques, jamais persistés en clair.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::TryRng;
use rand::rngs::SysRng;
use sha2::{Digest, Sha256};

use crate::error::AppError;

/// Entropie d'un jeton en octets.
pub const TOKEN_BYTES: usize = 32;
/// Longueur base64url sans padding d'un jeton de 32 octets.
pub const TOKEN_LEN: usize = 43;

/// 32 octets aléatoires du système, base64url sans padding.
pub fn random_token() -> Result<String, AppError> {
    let mut bytes = [0u8; TOKEN_BYTES];
    let mut rng = SysRng;
    rng.try_fill_bytes(&mut bytes)
        .map_err(|error| AppError::Invalid(format!("random source failure: {error}")))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// SHA-256 hexadécimal minuscule, format stocké par le store pour sessions et jetons.
pub fn hash_token(token: &str) -> String {
    use std::fmt::Write as _;
    let digest = Sha256::digest(token.as_bytes());
    let mut hex = String::with_capacity(64);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Base64url sans padding, 43 caractères décodant exactement 32 octets.
pub fn is_valid_token_format(token: &str) -> bool {
    token.len() == TOKEN_LEN
        && URL_SAFE_NO_PAD
            .decode(token)
            .is_ok_and(|bytes| bytes.len() == TOKEN_BYTES)
}

#[cfg(test)]
mod tests;
