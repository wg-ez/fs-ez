//! Options globales reconnues n'importe où dans la ligne.

use std::path::PathBuf;

use crate::error::CliError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobalOptions {
    pub root: Option<PathBuf>,
    pub db: Option<PathBuf>,
    pub json: bool,
    pub quiet: bool,
    pub verbose: bool,
    pub no_color: bool,
    pub yes: bool,
}

/// Argument restant après extraction, `literal` = protégé par `--`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Arg {
    pub text: String,
    pub literal: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Extracted {
    pub globals: GlobalOptions,
    pub help: bool,
    pub version: bool,
    pub args: Vec<Arg>,
}

/// Sépare `--nom=valeur` en nom et valeur inline.
pub(crate) fn split_long(raw: &str) -> Option<(&str, Option<&str>)> {
    if raw == "--" || !raw.starts_with("--") {
        return None;
    }
    match raw.split_once('=') {
        Some((name, value)) => Some((name, Some(value))),
        None => Some((raw, None)),
    }
}

fn missing_value(name: &str) -> CliError {
    CliError::Usage(format!("{name} requires a value"))
}

fn global_value(
    args: &[String],
    index: usize,
    inline: Option<&str>,
    name: &str,
) -> Result<(String, usize), CliError> {
    if let Some(value) = inline {
        if value.is_empty() {
            return Err(missing_value(name));
        }
        return Ok((value.to_string(), index + 1));
    }
    match args.get(index + 1) {
        Some(value) => Ok((value.clone(), index + 2)),
        None => Err(missing_value(name)),
    }
}

pub(crate) fn extract(args: &[String]) -> Result<Extracted, CliError> {
    let mut out = Extracted::default();
    let mut seen_command = false;
    let mut index = 0;
    while index < args.len() {
        let raw = &args[index];
        if raw == "--" {
            for rest in &args[index + 1..] {
                out.args.push(Arg {
                    text: rest.clone(),
                    literal: true,
                });
            }
            return Ok(out);
        }
        if let Some((name, inline)) = split_long(raw) {
            match name {
                "--root" | "--db" => {
                    let (value, next) = global_value(args, index, inline, name)?;
                    if name == "--root" {
                        out.globals.root = Some(PathBuf::from(value));
                    } else {
                        out.globals.db = Some(PathBuf::from(value));
                    }
                    index = next;
                    continue;
                }
                "--json" if inline.is_none() => out.globals.json = true,
                "--quiet" if inline.is_none() => out.globals.quiet = true,
                "--verbose" if inline.is_none() => out.globals.verbose = true,
                "--no-color" if inline.is_none() => out.globals.no_color = true,
                "--yes" if inline.is_none() => out.globals.yes = true,
                "--help" if inline.is_none() => out.help = true,
                "--version" if inline.is_none() => out.version = true,
                _ => {
                    if !seen_command {
                        return Err(CliError::Usage(format!("unknown global option '{raw}'")));
                    }
                    out.args.push(Arg {
                        text: raw.clone(),
                        literal: false,
                    });
                }
            }
            index += 1;
            continue;
        }
        if !seen_command && !raw.starts_with('-') {
            seen_command = true;
        }
        out.args.push(Arg {
            text: raw.clone(),
            literal: false,
        });
        index += 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
