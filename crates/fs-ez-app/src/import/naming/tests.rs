use super::*;

#[test]
fn sanitize_keeps_last_component_and_strips_leading_dots() {
    assert_eq!(
        sanitize_filename("../../etc/passwd").expect("nom"),
        "passwd"
    );
    assert_eq!(sanitize_filename(".hidden").expect("nom"), "hidden");
    assert_eq!(sanitize_filename("a/../b/c.txt").expect("nom"), "c.txt");
    assert_eq!(sanitize_filename("a\\b.txt").expect("nom"), "a_b.txt");
    assert_eq!(sanitize_filename("a\0b.txt").expect("nom"), "a_b.txt");
    assert_eq!(sanitize_filename("a\nb.txt").expect("nom"), "a_b.txt");
}

#[test]
fn sanitize_rejects_empty_dot_and_useless_names() {
    for name in ["", ".", "..", "...", "/", "___", "\\"] {
        assert!(sanitize_filename(name).is_err(), "{name:?}");
    }
}

#[test]
fn sanitize_limits_to_two_hundred_bytes_and_keeps_extension() {
    let long = format!("{}.txt", "é".repeat(150));
    let name = sanitize_filename(&long).expect("nom");
    assert!(name.len() <= 200);
    assert!(name.ends_with(".txt"));
    assert!(name.starts_with('é'));

    let no_ext = sanitize_filename(&"a".repeat(300)).expect("nom");
    assert_eq!(no_ext.len(), 200);
}

#[test]
fn suffix_goes_before_the_extension() {
    assert_eq!(
        suffixed_name("photo.tar.gz", "abc123"),
        "photo.tar-abc123.gz"
    );
    assert_eq!(suffixed_name("README", "abc123"), "README-abc123");
    assert_eq!(random_suffix().expect("hex").len(), 6);
}
