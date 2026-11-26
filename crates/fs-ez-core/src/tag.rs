//! Tags et applications sur les fichiers.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::codec::token_enum;
use crate::error::CoreError;
use crate::file::FileRecord;
use crate::ids::TagId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TagColor {
    Neutral,
    Red,
    Orange,
    Amber,
    Green,
    Teal,
    Cyan,
    Blue,
    Indigo,
    Violet,
    Pink,
    Brown,
}

impl TagColor {
    pub const ALL: [TagColor; 12] = [
        TagColor::Neutral,
        TagColor::Red,
        TagColor::Orange,
        TagColor::Amber,
        TagColor::Green,
        TagColor::Teal,
        TagColor::Cyan,
        TagColor::Blue,
        TagColor::Indigo,
        TagColor::Violet,
        TagColor::Pink,
        TagColor::Brown,
    ];
}

token_enum!(ref TagColor, as_token, parse,
    Neutral => "neutral",
    Red => "red",
    Orange => "orange",
    Amber => "amber",
    Green => "green",
    Teal => "teal",
    Cyan => "cyan",
    Blue => "blue",
    Indigo => "indigo",
    Violet => "violet",
    Pink => "pink",
    Brown => "brown",
);

/// Valide `#rrggbb` ou `#RGB` → `#rrggbb` en minuscules, refuse le reste.
pub fn validate_hex_color(raw: &str) -> Result<String, CoreError> {
    let invalid = || CoreError::InvalidColor(raw.to_string());
    let hex = raw.strip_prefix('#').ok_or_else(invalid)?;
    let digits: String = match hex.len() {
        3 => hex.chars().flat_map(|c| [c, c]).collect(),
        6 => hex.to_string(),
        _ => return Err(invalid()),
    };
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    Ok(format!("#{}", digits.to_lowercase()))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tag {
    pub id: TagId,
    pub name: String,
    pub name_norm: String,
    pub color: TagColor,
    pub color_custom: Option<String>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppliedTag {
    pub tag: Tag,
    pub explicit: bool,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileWithTags {
    pub file: FileRecord,
    pub tags: Vec<AppliedTag>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagCount {
    pub tag: Tag,
    pub count: u64,
}

#[cfg(test)]
mod tests;
