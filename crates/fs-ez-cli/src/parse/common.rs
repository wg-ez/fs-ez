//! Helpers de parsing partagés par les familles de commandes.

use fs_ez_core::{ApiTokenId, Sort, SortDir};

use crate::error::CliError;
use crate::globals::{self, Arg};

pub(super) fn usage(message: impl Into<String>) -> CliError {
    CliError::Usage(message.into())
}

pub(super) fn option(arg: &Arg) -> Option<(&str, Option<&str>)> {
    if arg.literal {
        None
    } else {
        globals::split_long(&arg.text)
    }
}

/// Token courant, un `Arg { literal: true }` reste positionnel.
pub(super) enum Token<'a> {
    Option {
        name: &'a str,
        inline: Option<&'a str>,
    },
    Positional(&'a Arg),
}

/// Curseur d'arguments, `token` ne consomme rien, `value`/`skip` avancent.
pub(super) struct Cursor<'a> {
    args: &'a [Arg],
    index: usize,
}

impl<'a> Cursor<'a> {
    pub(super) fn new(args: &'a [Arg]) -> Self {
        Self { args, index: 0 }
    }

    pub(super) fn token(&self) -> Option<Token<'a>> {
        let arg = self.args.get(self.index)?;
        match option(arg) {
            Some((name, inline)) => Some(Token::Option { name, inline }),
            None => Some(Token::Positional(arg)),
        }
    }

    /// Même sémantique d'index que `take_value` : inline +1, séparé +2.
    pub(super) fn value(&mut self, name: &str, inline: Option<&str>) -> Result<String, CliError> {
        if let Some(value) = inline {
            self.index += 1;
            return Ok(value.to_string());
        }
        match self.args.get(self.index + 1) {
            Some(arg) => {
                self.index += 2;
                Ok(arg.text.clone())
            }
            None => Err(usage(format!("{name} requires a value"))),
        }
    }

    pub(super) fn skip(&mut self) {
        self.index += 1;
    }

    /// Collecte les positionnels, `on_option` traite chaque option dans l'ordre.
    pub(super) fn parse<F>(&mut self, mut on_option: F) -> Result<Vec<String>, CliError>
    where
        F: FnMut(&mut Cursor<'a>, &str, Option<&str>) -> Result<(), CliError>,
    {
        let mut positionals = Vec::new();
        while let Some(token) = self.token() {
            match token {
                Token::Option { name, inline } => on_option(self, name, inline)?,
                Token::Positional(arg) => {
                    positionals.push(arg.text.clone());
                    self.skip();
                }
            }
        }
        Ok(positionals)
    }

    /// Consomme l'option si elle est nue, `false` laisse le fallback `unknown option`.
    pub(super) fn flag(&mut self, inline: Option<&str>) -> bool {
        if inline.is_some() {
            return false;
        }
        self.skip();
        true
    }
}

/// Nom + `--color` de `group add` / `view-group add`, 2e positionnel refusé tôt.
pub(super) fn parse_name_color(
    args: &[Arg],
    family: &str,
) -> Result<(String, Option<String>), CliError> {
    let mut name = None;
    let mut color = None;
    let mut cursor = Cursor::new(args);
    while let Some(token) = cursor.token() {
        match token {
            Token::Option {
                name: option_name,
                inline,
            } => match option_name {
                "--color" => color = Some(cursor.value("--color", inline)?),
                _ => return Err(usage(format!("unknown option '{option_name}'"))),
            },
            Token::Positional(arg) => {
                if name.is_some() {
                    return Err(usage(format!("{family} add expects exactly one name")));
                }
                name = Some(arg.text.clone());
                cursor.skip();
            }
        }
    }
    let name = name.ok_or_else(|| usage(format!("{family} add expects a name")))?;
    Ok((name, color))
}

pub(super) fn push_tags(tags: &mut Vec<String>, raw: &str) {
    for tag in raw.split(',') {
        if !tag.is_empty() {
            tags.push(tag.to_string());
        }
    }
}

pub(super) fn reject_options(args: &[Arg]) -> Result<Vec<String>, CliError> {
    args.iter()
        .map(|arg| match option(arg) {
            Some((name, _)) => Err(usage(format!("unknown option '{name}'"))),
            None => Ok(arg.text.clone()),
        })
        .collect()
}

/// `reject_options` d'abord, arité ensuite, messages de `message` si le compte diffère.
pub(super) fn exact_args<const N: usize>(
    args: &[Arg],
    message: &str,
) -> Result<[String; N], CliError> {
    let values = reject_options(args)?;
    values.try_into().map_err(|_| usage(message))
}

pub(super) fn ensure_empty(args: &[Arg], command: &str) -> Result<(), CliError> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(usage(format!("{command} takes no arguments")))
    }
}

pub(super) fn parse_limit(raw: &str) -> Result<usize, CliError> {
    let limit = parse_number::<usize>(raw, "limit")?;
    fs_ez_store::validate_page_limit(limit).map_err(|_| usage(format!("invalid limit '{raw}'")))?;
    Ok(limit)
}

pub(super) fn parse_offset(raw: &str) -> Result<usize, CliError> {
    parse_number::<usize>(raw, "offset")
}

pub(super) fn parse_sort(raw: &str) -> Result<Sort, CliError> {
    Sort::decode_with_default_dir(raw, SortDir::Asc)
        .ok_or_else(|| usage(format!("invalid sort '{raw}'")))
}

pub(super) fn parse_number<T: std::str::FromStr + Copy>(
    raw: &str,
    label: &str,
) -> Result<T, CliError> {
    raw.parse::<T>()
        .map_err(|_| usage(format!("invalid {label} '{raw}'")))
}

pub(super) fn parse_token_id(raw: &str) -> Result<ApiTokenId, CliError> {
    let value = parse_number::<i64>(raw, "token id")?;
    if value <= 0 {
        return Err(usage(format!("invalid token id '{raw}'")));
    }
    Ok(ApiTokenId(value))
}
