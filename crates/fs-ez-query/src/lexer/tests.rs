use super::*;

fn tokens(input: &str) -> Vec<Token> {
    lex(input)
        .expect("lexing valide")
        .into_iter()
        .map(|spanned| spanned.token)
        .collect()
}

#[test]
fn rejects_comparison_symbols() {
    for (input, position) in [("a!=b", 1), ("a<=b", 1), ("a>b", 1), ("a=b", 1)] {
        assert!(matches!(
            lex(input),
            Err(QueryError::UnexpectedToken(_, pos)) if pos == position
        ));
    }
}

#[test]
fn lexes_unicode_identifiers_and_keywords_case_insensitively() {
    assert_eq!(
        tokens("Été AnD NOT(x)oR only y"),
        vec![
            Token::Ident("Été".to_string()),
            Token::And,
            Token::Not,
            Token::LParen,
            Token::Ident("x".to_string()),
            Token::RParen,
            Token::Or,
            Token::Ident("only".to_string()),
            Token::Ident("y".to_string()),
            Token::Eof
        ]
    );
    // Un mot-clé n'est reconnu que s'il forme le jeton entier.
    assert_eq!(
        tokens("anderson order noteworthy"),
        vec![
            Token::Ident("anderson".to_string()),
            Token::Ident("order".to_string()),
            Token::Ident("noteworthy".to_string()),
            Token::Eof
        ]
    );
}

#[test]
fn keeps_colons_and_dashes_in_identifiers() {
    assert_eq!(
        tokens("projet:client-a"),
        vec![Token::Ident("projet:client-a".to_string()), Token::Eof]
    );
}

#[test]
fn decodes_quoted_escapes() {
    assert_eq!(
        tokens(r#""a\"b\\c\n""#),
        vec![Token::Quoted("a\"b\\c\n".to_string()), Token::Eof]
    );
    assert_eq!(
        tokens(r#""""#),
        vec![Token::Quoted(String::new()), Token::Eof]
    );
}

#[test]
fn rejects_unterminated_quoted_string() {
    assert_eq!(lex("\"abc"), Err(QueryError::UnexpectedEnd));
    assert_eq!(lex("\"abc\\"), Err(QueryError::UnexpectedEnd));
}

#[test]
fn rejects_unknown_escape() {
    assert_eq!(
        lex(r#""a\q""#),
        Err(QueryError::UnexpectedToken("\\q".to_string(), 2))
    );
}

#[test]
fn rejects_isolated_bang_ampersand_pipe() {
    assert_eq!(
        lex("a ! b"),
        Err(QueryError::UnexpectedToken("!".to_string(), 2))
    );
    assert_eq!(
        lex("a & b"),
        Err(QueryError::UnexpectedToken("&".to_string(), 2))
    );
    assert_eq!(
        lex("a | b"),
        Err(QueryError::UnexpectedToken("|".to_string(), 2))
    );
    // `&` et `|` ne sont réservés que lorsqu'ils sont isolés : collés, l'identifiant
    // est ensuite refusé par la normalisation (`QueryError::InvalidName`).
    assert_eq!(
        lex("a|b").expect("lexing valide")[0].token,
        Token::Ident("a|b".to_string())
    );
}

#[test]
fn reports_byte_positions() {
    let spanned = lex("é  b").expect("lexing valide");
    let positions: Vec<usize> = spanned.iter().map(|token| token.pos).collect();
    // « é » occupe deux octets, d'où le décalage des positions suivantes.
    assert_eq!(positions, vec![0, 4, 5]);
    assert_eq!(spanned[2].token, Token::Eof);
}

#[test]
fn rejects_too_many_tokens() {
    let input = "a ".repeat(MAX_TOKENS + 1);
    assert_eq!(lex(&input), Err(QueryError::TooComplex));
}
