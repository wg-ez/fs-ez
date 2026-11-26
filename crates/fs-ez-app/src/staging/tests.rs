use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use super::*;

#[test]
fn dav_part_keeps_label_and_token_verbatim() {
    assert_eq!(dav_part("42", "abcDEF_-"), "dav-42-abcDEF_-.part");
    assert_eq!(dav_part("name", ""), "dav-name-.part");
}

#[test]
fn content_part_keeps_the_leading_dot() {
    assert_eq!(content_part("token"), ".dav-token.part");
    assert!(!is_temp_file_name(OsStr::new(&content_part("token"))));
}

#[test]
fn dav_hash_path_is_relative_under_the_dav_dir() {
    let path = dav_hash_path("aa", "bb");
    assert_eq!(path, PathBuf::from("dav").join("aa").join("bb"));
    assert!(!path.is_absolute());
}

#[test]
fn temp_names_require_prefix_and_part_suffix() {
    assert!(is_temp_file_name(OsStr::new("upload-x.part")));
    assert!(is_temp_file_name(OsStr::new("dav-label-token.part")));
    assert!(!is_temp_file_name(OsStr::new("dav.part")));
    assert!(!is_temp_file_name(OsStr::new("upload-x.tmp")));
    assert!(!is_temp_file_name(OsStr::new("x.part")));
    assert!(!is_temp_file_name(OsStr::new(".dav-x.part")));
}

#[test]
fn temp_names_reject_non_utf8() {
    assert!(!is_temp_file_name(OsStr::from_bytes(b"upload-\xff.part")));
}

#[test]
fn hash_names_are_64_lowercase_hex_only() {
    assert!(is_hash_name(OsStr::new(&"a".repeat(HASH_LEN))));
    assert!(is_hash_name(OsStr::new(&"0".repeat(HASH_LEN))));
    assert!(!is_hash_name(OsStr::new(&"a".repeat(HASH_LEN - 1))));
    assert!(!is_hash_name(OsStr::new(&"A".repeat(HASH_LEN))));
    assert!(!is_hash_name(OsStr::new(&"g".repeat(HASH_LEN))));
    assert!(!is_hash_name(OsStr::new("dav")));
}
