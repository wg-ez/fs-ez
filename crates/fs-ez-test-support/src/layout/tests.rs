//! Tests du contrôle de disposition.

use super::*;

#[test]
fn test_file_detection() {
    assert!(is_test_file(Path::new("/a/src/foo/tests.rs")));
    assert!(is_test_file(Path::new("/a/src/foo/tests/mod.rs")));
    assert!(is_test_file(Path::new("/a/src/foo/bar_test.rs")));
    assert!(!is_test_file(Path::new("/a/src/foo.rs")));
}

#[test]
fn inline_module_detection() {
    assert_eq!(
        declares_inline_test_module(&["#[cfg(test)]", "mod tests {"]),
        Some(0)
    );
    assert_eq!(
        declares_inline_test_module(&["#[cfg(test)] mod tests {"]),
        Some(0)
    );
    assert_eq!(
        declares_inline_test_module(&["#[cfg(test)]", "mod tests;"]),
        None
    );
}

#[test]
fn test_function_detection() {
    assert_eq!(test_function_lines(&["#[test]", "fn a() {}"]), vec![0]);
    assert_eq!(
        test_function_lines(&["#[tokio::test]", "async fn a() {}"]),
        vec![0]
    );
}
