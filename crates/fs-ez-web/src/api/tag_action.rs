//! Corps et action communs aux mutations de tags des fichiers et de l'inbox.

use serde::Deserialize;

pub(crate) use crate::forms::TagAction;

#[derive(Debug, Deserialize)]
pub struct TagsBody {
    #[serde(default)]
    pub tags: Vec<String>,
    pub action: String,
}
