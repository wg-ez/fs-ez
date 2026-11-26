use super::*;

#[test]
fn accepts_at_most_max_tags() {
    let raw: Vec<String> = (0..MAX_TAGS_PER_REQUEST)
        .map(|i| format!("tag{i}"))
        .collect();
    let parsed = parse_tag_names(&raw).expect("tags");
    assert_eq!(parsed.len(), MAX_TAGS_PER_REQUEST);
}

#[test]
fn rejects_more_than_max_tags() {
    let raw: Vec<String> = (0..=MAX_TAGS_PER_REQUEST)
        .map(|i| format!("tag{i}"))
        .collect();
    match parse_tag_names(&raw) {
        Err(AppError::Invalid(message)) => assert!(message.contains("too many tags")),
        other => panic!("attendu Invalid, obtenu {other:?}"),
    }
}

#[test]
fn deduplicates_case_insensitively_and_trims() {
    let raw = vec!["  X ".to_string(), "x".to_string(), " Yy ".to_string()];
    let parsed = parse_tag_names(&raw).expect("tags");
    assert_eq!(parsed, vec!["X".to_string(), "Yy".to_string()]);
}

#[test]
fn rejects_invalid_names() {
    assert!(parse_tag_names(&["a,b".to_string()]).is_err());
    assert!(parse_tag_names(&["".to_string()]).is_err());
}
