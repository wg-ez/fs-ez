//! Lexer TMSU : découpe une requête en jetons positionnés en octets.

use crate::error::QueryError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Ident(String),
    Quoted(String),
    And,
    Or,
    Not,
    LParen,
    RParen,
    Eof,
}

impl Token {
    /// Forme lisible pour les messages d'erreur.
    pub fn describe(&self) -> String {
        match self {
            Self::Ident(text) => text.clone(),
            Self::Quoted(text) => format!("\"{text}\""),
            Self::And => "and".to_string(),
            Self::Or => "or".to_string(),
            Self::Not => "not".to_string(),
            Self::LParen => "(".to_string(),
            Self::RParen => ")".to_string(),
            Self::Eof => "end of query".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedToken {
    pub token: Token,
    pub pos: usize,
}

/// Termine un identifiant non quoté, l'espace reste un séparateur de requête.
const RESERVED: &[char] = &['\t', '\n', '\r', '(', ')', '=', '!', '<', '>', '"'];

/// Plafond de jetons produits pour borner la mémoire et le travail du lexer.
pub(crate) const MAX_TOKENS: usize = 4096;

fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

pub fn lex(input: &str) -> Result<Vec<SpannedToken>, QueryError> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();
    while let Some((pos, c)) = chars.next() {
        if is_space(c) {
            continue;
        }
        let token = match c {
            '(' => Token::LParen,
            ')' => Token::RParen,
            '=' | '!' | '<' | '>' | '&' | '|' => {
                return Err(QueryError::UnexpectedToken(c.to_string(), pos));
            }
            '"' => read_quoted(&mut chars)?,
            _ => read_ident(&mut chars, c),
        };
        if tokens.len() >= MAX_TOKENS {
            return Err(QueryError::TooComplex);
        }
        tokens.push(SpannedToken { token, pos });
    }
    tokens.push(SpannedToken {
        token: Token::Eof,
        pos: input.len(),
    });
    Ok(tokens)
}

fn read_ident(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>, first: char) -> Token {
    let mut ident = String::from(first);
    while let Some((_, c)) = chars.peek() {
        if is_space(*c) || RESERVED.contains(c) {
            break;
        }
        ident.push(*c);
        chars.next();
    }
    if ident.eq_ignore_ascii_case("and") {
        Token::And
    } else if ident.eq_ignore_ascii_case("or") {
        Token::Or
    } else if ident.eq_ignore_ascii_case("not") {
        Token::Not
    } else {
        Token::Ident(ident)
    }
}

fn read_quoted(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
) -> Result<Token, QueryError> {
    let mut text = String::new();
    while let Some((pos, c)) = chars.next() {
        match c {
            '"' => return Ok(Token::Quoted(text)),
            '\\' => match chars.next() {
                Some((_, '"')) => text.push('"'),
                Some((_, '\\')) => text.push('\\'),
                Some((_, 'n')) => text.push('\n'),
                Some((_, other)) => {
                    return Err(QueryError::UnexpectedToken(format!("\\{other}"), pos));
                }
                None => return Err(QueryError::UnexpectedEnd),
            },
            other => text.push(other),
        }
    }
    Err(QueryError::UnexpectedEnd)
}

#[cfg(test)]
mod tests;
