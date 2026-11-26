//! Réglages typés et préférences UI, conversion clé/valeur sans I/O.

use serde::{Deserialize, Serialize};

use crate::codec::token_enum;
use crate::error::CoreError;
use crate::ids::GroupId;
use crate::view::{Sort, SortDir, SortField};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Light,
    Dark,
    System,
}

token_enum!(ref Theme, as_token, parse,
    Light => "light",
    Dark => "dark",
    System => "system",
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    En,
    Fr,
}

token_enum!(ref Language, as_token, parse,
    En => "en",
    Fr => "fr",
);

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub recent_days: i64,
    pub recent_file_days: i64,
    pub default_sort: Sort,
    pub page_size: usize,
    pub sidebar_collapsed: bool,
    pub open_groups: Vec<GroupId>,
    pub theme: Theme,
    pub language: Language,
    pub files_dir: String,
    pub backup_days: i64,
    pub backup_hour: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            recent_days: 7,
            recent_file_days: 7,
            default_sort: Sort::default(),
            page_size: 100,
            sidebar_collapsed: false,
            open_groups: Vec::new(),
            theme: Theme::Light,
            language: Language::En,
            files_dir: "files".to_string(),
            backup_days: 7,
            backup_hour: 3,
        }
    }
}

type SettingGetter = fn(&Settings) -> String;
type SettingSetter = fn(&str, &str, &mut Settings) -> Result<(), CoreError>;

/// `to_pairs` et `set` générés depuis les mêmes clés, getters et validateurs.
macro_rules! settings_table {
    ($($key:literal => $get:expr, $set:expr;)+) => {
        pub fn to_pairs(&self) -> Vec<(String, String)> {
            let getters: &[(&str, SettingGetter)] = &[$(( $key, $get )),+];
            getters
                .iter()
                .map(|(key, get)| ((*key).to_string(), get(self)))
                .collect()
        }

        pub fn set(&mut self, key: &str, value: &str) -> Result<(), CoreError> {
            let setters: &[(&str, SettingSetter)] = &[$(( $key, $set )),+];
            match setters.iter().find(|(candidate, _)| *candidate == key) {
                Some((_, set)) => set(key, value, self),
                None => Err(CoreError::InvalidName(format!("unknown setting: {key}"))),
            }
        }
    };
}

impl Settings {
    /// Valeur invalide ignorée : on garde le défaut, jamais de panique.
    pub fn from_pairs<'a>(pairs: impl Iterator<Item = (&'a str, &'a str)>) -> Self {
        let mut settings = Self::default();
        for (key, value) in pairs {
            let _ = settings.set(key, value);
        }
        settings
    }

    settings_table! {
        "ui.recent_days" => |s: &Settings| s.recent_days.to_string(),
            |key: &str, value: &str, s: &mut Settings| -> Result<(), CoreError> {
                s.recent_days = parse_range(value, 1, 365).ok_or_else(|| invalid(key, value))?;
                Ok(())
            };
        "ui.recent_file_days" => |s: &Settings| s.recent_file_days.to_string(),
            |key: &str, value: &str, s: &mut Settings| -> Result<(), CoreError> {
                s.recent_file_days =
                    parse_range(value, 1, 365).ok_or_else(|| invalid(key, value))?;
                Ok(())
            };
        "ui.sort_field" => |s: &Settings| s.default_sort.field.as_token().to_string(),
            |key: &str, value: &str, s: &mut Settings| -> Result<(), CoreError> {
                s.default_sort.field =
                    SortField::from_token(value).ok_or_else(|| invalid(key, value))?;
                Ok(())
            };
        "ui.sort_dir" => |s: &Settings| s.default_sort.dir.as_token().to_string(),
            |key: &str, value: &str, s: &mut Settings| -> Result<(), CoreError> {
                s.default_sort.dir =
                    SortDir::from_token(value).ok_or_else(|| invalid(key, value))?;
                Ok(())
            };
        "ui.page_size" => |s: &Settings| s.page_size.to_string(),
            |key: &str, value: &str, s: &mut Settings| -> Result<(), CoreError> {
                let size = parse_range(value, 1, 500).ok_or_else(|| invalid(key, value))?;
                s.page_size = usize::try_from(size).map_err(|_| invalid(key, value))?;
                Ok(())
            };
        "ui.sidebar_collapsed" => |s: &Settings| if s.sidebar_collapsed { "1" } else { "0" }.to_string(),
            |key: &str, value: &str, s: &mut Settings| -> Result<(), CoreError> {
                s.sidebar_collapsed = match value {
                    "0" => false,
                    "1" => true,
                    _ => return Err(invalid(key, value)),
                };
                Ok(())
            };
        "ui.open_groups" => |s: &Settings| {
            s.open_groups
                .iter()
                .map(|g| g.0.to_string())
                .collect::<Vec<_>>()
                .join(",")
        },
            |key: &str, value: &str, s: &mut Settings| -> Result<(), CoreError> {
                s.open_groups = parse_groups(value).ok_or_else(|| invalid(key, value))?;
                Ok(())
            };
        "ui.theme" => |s: &Settings| s.theme.as_token().to_string(),
            |key: &str, value: &str, s: &mut Settings| -> Result<(), CoreError> {
                s.theme = Theme::parse(value).ok_or_else(|| invalid(key, value))?;
                Ok(())
            };
        "ui.language" => |s: &Settings| s.language.as_token().to_string(),
            |key: &str, value: &str, s: &mut Settings| -> Result<(), CoreError> {
                s.language = Language::parse(value).ok_or_else(|| invalid(key, value))?;
                Ok(())
            };
        "io.files_dir" => |s: &Settings| s.files_dir.clone(),
            |key: &str, value: &str, s: &mut Settings| -> Result<(), CoreError> {
                s.files_dir = validate_relative_dir(value).map_err(|_| invalid(key, value))?;
                Ok(())
            };
        "io.backup_days" => |s: &Settings| s.backup_days.to_string(),
            |key: &str, value: &str, s: &mut Settings| -> Result<(), CoreError> {
                s.backup_days = parse_range(value, 1, 365).ok_or_else(|| invalid(key, value))?;
                Ok(())
            };
        "io.backup_hour" => |s: &Settings| s.backup_hour.to_string(),
            |key: &str, value: &str, s: &mut Settings| -> Result<(), CoreError> {
                s.backup_hour = parse_range(value, 0, 23).ok_or_else(|| invalid(key, value))?;
                Ok(())
            };
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.to_pairs()
            .into_iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }
}

fn invalid(key: &str, value: &str) -> CoreError {
    CoreError::InvalidName(format!("setting {key}: invalid value {value:?}"))
}

fn parse_range(value: &str, min: i64, max: i64) -> Option<i64> {
    let parsed: i64 = value.parse().ok()?;
    (min..=max).contains(&parsed).then_some(parsed)
}

fn parse_groups(value: &str) -> Option<Vec<GroupId>> {
    if value.is_empty() {
        return Some(Vec::new());
    }
    value
        .split(',')
        .map(|part| part.trim().parse::<i64>().ok().map(GroupId))
        .collect()
}

/// Chemin relatif sans absolu, sans `.`, sans `..`, sans composant vide.
fn validate_relative_dir(value: &str) -> Result<String, CoreError> {
    let rejected = || CoreError::InvalidName(format!("invalid relative dir {value:?}"));
    let normalized = crate::path::normalize_relative_path(value).ok_or_else(rejected)?;
    Ok(normalized.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests;
