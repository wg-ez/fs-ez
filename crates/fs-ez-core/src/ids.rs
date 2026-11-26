//! Identifiants typés des entités du domaine.

use serde::{Deserialize, Serialize};

macro_rules! define_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        /// Invariant : strictement positif, `0` et négatifs refusés à la désérialisation.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
        #[serde(transparent)]
        pub struct $name(pub i64);

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = i64::deserialize(deserializer)?;
                if value > 0 {
                    Ok(Self(value))
                } else {
                    Err(serde::de::Error::custom("id must be positive"))
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl From<i64> for $name {
            fn from(value: i64) -> Self {
                Self(value)
            }
        }

        impl From<$name> for i64 {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

define_id!(FileId, "Identifiant d'un fichier référencé.");
define_id!(TagId, "Identifiant d'un tag.");
define_id!(GroupId, "Identifiant d'un groupe de tags.");
define_id!(ViewId, "Identifiant d'une vue sauvegardée.");
define_id!(ViewGroupId, "Identifiant d'un groupe de vues.");
define_id!(UserId, "Identifiant du compte utilisateur.");
define_id!(SessionId, "Identifiant d'une session web.");
define_id!(ApiTokenId, "Identifiant d'un jeton d'API.");
define_id!(VirtualFolderId, "Identifiant d'un dossier virtuel.");

#[cfg(test)]
mod tests;
