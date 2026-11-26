use super::*;

fn pairs(entries: &[(&str, &str)]) -> Vec<(String, String)> {
    entries
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect()
}

#[test]
fn absent_sort_uses_the_default() {
    let input = parse_input(&pairs(&[("name", "Vue")])).expect("tri par défaut");
    assert_eq!(input.sort, Sort::default());
}

#[test]
fn blank_sort_uses_the_default() {
    let input = parse_input(&pairs(&[("name", "Vue"), ("sort", "   ")])).expect("tri par défaut");
    assert_eq!(input.sort, Sort::default());
}

#[test]
fn invalid_sort_is_rejected() {
    let error =
        parse_input(&pairs(&[("name", "Vue"), ("sort", "bogus")])).expect_err("tri invalide");
    match error {
        WebError::BadRequest(message) => assert!(message.contains("invalid sort"), "{message}"),
        other => panic!("erreur inattendue : {other:?}"),
    }
}

#[test]
fn valid_sort_is_parsed() {
    let input = parse_input(&pairs(&[("name", "Vue"), ("sort", "name:desc")])).expect("tri valide");
    assert_eq!(input.sort.field, fs_ez_core::SortField::Name);
    assert_eq!(input.sort.dir, fs_ez_core::SortDir::Desc);
}
