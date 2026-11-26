use super::*;

#[test]
fn files_query_sort_and_limit() {
    let result = parsed(&["files", "a and b", "--sort=name:desc", "--limit=5"]);
    match result.command {
        Command::Files {
            query, sort, limit, ..
        } => {
            assert_eq!(query.as_deref(), Some("a and b"));
            assert_eq!(
                sort,
                Sort {
                    field: SortField::Name,
                    dir: SortDir::Desc
                }
            );
            assert_eq!(limit, 5);
        }
        other => panic!("unexpected command {other:?}"),
    }
}

#[test]
fn files_joins_remaining_words() {
    let result = parsed(&["files", "alpha", "and", "beta"]);
    assert!(matches!(
        result.command,
        Command::Files { query: Some(ref query), .. } if query == "alpha and beta"
    ));
}

#[test]
fn files_limit_zero_is_refused() {
    assert!(matches!(
        failed(&["files", "--limit=0"]),
        CliError::Usage(_)
    ));
}

#[test]
fn files_bad_sort_is_refused() {
    assert!(matches!(
        failed(&["files", "--sort=nope"]),
        CliError::Usage(_)
    ));
}

#[test]
fn aliases_produce_canonical_variants() {
    assert!(matches!(parsed(&["ls"]).command, Command::Files { .. }));
    assert!(matches!(
        parsed(&["find", "alpha"]).command,
        Command::Files { .. }
    ));
    assert!(matches!(
        parsed(&["rm", "a"]).command,
        Command::Delete { .. }
    ));
}
