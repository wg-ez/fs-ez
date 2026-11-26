#![forbid(unsafe_code)]
//! Langage de requêtes TMSU : lexer, parser, AST, pretty-printer et validation.

pub mod error;
pub mod eval;
pub mod lexer;
pub mod parser;
pub mod pretty;

pub use error::QueryError;
pub use eval::{EvalTag, evaluate};
pub use lexer::{SpannedToken, Token, lex};
pub use parser::parse;
pub use pretty::pretty;

#[cfg(test)]
mod layout_test;
