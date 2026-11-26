//! Parser TMSU : jeton → `FilterExpr`, avec validation du domaine.

use fs_ez_core::{CoreError, FilterExpr, normalize_tag_name};

use crate::error::QueryError;
use crate::lexer::{SpannedToken, Token, lex};

/// Priorité décroissante : `or` < `and` < primaire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    And,
    Or,
}

/// Plafond de récursion : 64 niveaux de parenthèses ou de `not`.
const MAX_PARSE_DEPTH: usize = 64;

pub fn parse(input: &str) -> Result<FilterExpr, QueryError> {
    if input.trim().is_empty() {
        return Err(QueryError::Empty);
    }
    let tokens = lex(input)?;
    let mut parser = Parser {
        tokens: &tokens,
        index: 0,
        depth: 0,
    };
    let expr = parser.parse_or()?;
    parser.expect_eof()?;
    expr.validate().map_err(map_filter_error)?;
    Ok(expr)
}

struct Parser<'a> {
    tokens: &'a [SpannedToken],
    index: usize,
    depth: usize,
}

const EOF: SpannedToken = SpannedToken {
    token: Token::Eof,
    pos: 0,
};

impl<'a> Parser<'a> {
    fn current(&self) -> &SpannedToken {
        // Le lexer termine toujours par `Eof` et `bump` s'arrête sur ce jeton.
        self.tokens.get(self.index).unwrap_or(&EOF)
    }

    fn bump(&mut self) {
        if self.index + 1 < self.tokens.len() {
            self.index += 1;
        }
    }

    fn is(&self, token: &Token) -> bool {
        &self.current().token == token
    }

    fn unexpected(&self) -> QueryError {
        if self.is(&Token::Eof) {
            QueryError::UnexpectedEnd
        } else {
            QueryError::UnexpectedToken(self.current().token.describe(), self.current().pos)
        }
    }

    /// Contrôle la profondeur avant toute descente récursive : `not` et `(`.
    fn nested<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<T, QueryError>,
    ) -> Result<T, QueryError> {
        if self.depth >= MAX_PARSE_DEPTH {
            return Err(QueryError::TooComplex);
        }
        self.depth += 1;
        let result = parse(self);
        self.depth -= 1;
        result
    }

    fn expect_eof(&self) -> Result<(), QueryError> {
        if self.is(&Token::Eof) {
            Ok(())
        } else {
            Err(self.unexpected())
        }
    }

    fn parse_or(&mut self) -> Result<FilterExpr, QueryError> {
        let mut items = vec![self.parse_and()?];
        while self.is(&Token::Or) {
            self.bump();
            items.push(self.parse_and()?);
        }
        Ok(combine(Op::Or, items))
    }

    fn parse_and(&mut self) -> Result<FilterExpr, QueryError> {
        let mut items = vec![self.parse_unary()?];
        while self.is(&Token::And) {
            self.bump();
            items.push(self.parse_unary()?);
        }
        Ok(combine(Op::And, items))
    }

    fn parse_unary(&mut self) -> Result<FilterExpr, QueryError> {
        if self.is(&Token::Not) {
            self.bump();
            let operand = self.nested(Self::parse_unary)?;
            return Ok(negate(operand));
        }
        self.parse_primary()
    }

    fn at_keyword(&self, keyword: &str) -> bool {
        matches!(&self.current().token, Token::Ident(text) if text.eq_ignore_ascii_case(keyword))
            && matches!(
                self.tokens.get(self.index + 1).map(|token| &token.token),
                Some(Token::Ident(_) | Token::Quoted(_) | Token::LParen)
            )
    }

    fn parse_primary(&mut self) -> Result<FilterExpr, QueryError> {
        if self.at_keyword("exactly") {
            return self.parse_tag_set();
        }
        if self.at_keyword("only") {
            self.bump();
            return Ok(FilterExpr::OnlyTag {
                tag: self.take_tag()?,
                negated: false,
            });
        }
        match &self.current().token {
            Token::LParen => {
                self.bump();
                if self.is(&Token::RParen) {
                    return Err(self.unexpected());
                }
                let inner = self.nested(Self::parse_or)?;
                if !self.is(&Token::RParen) {
                    return Err(self.unexpected());
                }
                self.bump();
                Ok(inner)
            }
            Token::Ident(_) | Token::Quoted(_) => self.parse_tag_term(),
            _ => Err(self.unexpected()),
        }
    }

    fn parse_tag_set(&mut self) -> Result<FilterExpr, QueryError> {
        self.bump();
        let mut tags = Vec::new();
        if self.is(&Token::LParen) {
            self.bump();
            loop {
                tags.push(self.take_tag()?);
                if self.is(&Token::And) {
                    self.bump();
                    continue;
                }
                if self.is(&Token::RParen) {
                    self.bump();
                    break;
                }
                return Err(self.unexpected());
            }
        } else {
            tags.push(self.take_tag()?);
        }
        Ok(FilterExpr::TagSet {
            tags,
            negated: false,
        })
    }

    fn parse_tag_term(&mut self) -> Result<FilterExpr, QueryError> {
        Ok(FilterExpr::Tag {
            tag: self.take_tag()?,
            negated: false,
        })
    }

    fn take_tag(&mut self) -> Result<String, QueryError> {
        let raw = self.take_name()?;
        normalize_tag(&raw)
    }

    fn take_name(&mut self) -> Result<String, QueryError> {
        let raw = match &self.current().token {
            Token::Ident(text) | Token::Quoted(text) => text.clone(),
            _ => return Err(self.unexpected()),
        };
        self.bump();
        Ok(raw)
    }
}

/// `not tag` se replie sur `Tag.negated`, `not not x` s'annule, le reste devient `Not`.
fn negate(expr: FilterExpr) -> FilterExpr {
    match expr {
        FilterExpr::Tag { tag, negated } => FilterExpr::Tag {
            tag,
            negated: !negated,
        },
        FilterExpr::OnlyTag { tag, negated } => FilterExpr::OnlyTag {
            tag,
            negated: !negated,
        },
        FilterExpr::TagSet { tags, negated } => FilterExpr::TagSet {
            tags,
            negated: !negated,
        },
        FilterExpr::Not { item } => *item,
        other => FilterExpr::Not {
            item: Box::new(other),
        },
    }
}

/// Aplatit les opérateurs identiques pour obtenir une forme canonique unique.
fn combine(op: Op, items: Vec<FilterExpr>) -> FilterExpr {
    let mut flat = Vec::with_capacity(items.len());
    for item in items {
        match (op, item) {
            (Op::And, FilterExpr::And { items: nested }) => flat.extend(nested),
            (Op::Or, FilterExpr::Or { items: nested }) => flat.extend(nested),
            (_, item) => flat.push(item),
        }
    }
    if flat.len() == 1
        && let Some(single) = flat.pop()
    {
        return single;
    }
    match op {
        Op::And => FilterExpr::And { items: flat },
        Op::Or => FilterExpr::Or { items: flat },
    }
}

fn normalize_tag(raw: &str) -> Result<String, QueryError> {
    normalize_tag_name(raw)
        .map(|(display, _)| display)
        .map_err(QueryError::from_core)
}

fn map_filter_error(err: CoreError) -> QueryError {
    match err {
        CoreError::InvalidFilter(message)
            if message.contains("depth") || message.contains("node count") =>
        {
            QueryError::TooComplex
        }
        CoreError::InvalidFilter(message) => QueryError::InvalidName(message),
        CoreError::InvalidName(name) => QueryError::InvalidName(name),
        other => QueryError::InvalidName(other.to_string()),
    }
}

#[cfg(test)]
mod tests;
