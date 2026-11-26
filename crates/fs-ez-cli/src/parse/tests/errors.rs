use super::*;

#[test]
fn usage_errors() {
    assert!(matches!(failed(&["tag", "path"]), CliError::Usage(_)));
    assert!(matches!(failed(&["tag", "--tags=a"]), CliError::Usage(_)));
    assert!(matches!(failed(&["merge", "source"]), CliError::Usage(_)));
    assert!(matches!(
        failed(&["token", "revoke", "x"]),
        CliError::Usage(message) if message.contains('x')
    ));
    assert!(matches!(
        failed(&["serve", "--port", "70000"]),
        CliError::Usage(_)
    ));
    assert!(matches!(failed(&[]), CliError::Usage(_)));
    assert!(matches!(failed(&["nope"]), CliError::Usage(_)));
    assert!(matches!(failed(&["--nope", "files"]), CliError::Usage(_)));
}
