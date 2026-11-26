use std::io::Cursor;

use super::*;

#[test]
fn read_password_reads_only_the_injected_reader() {
    let mut input = Cursor::new(b"correct horse battery\n".to_vec());
    assert_eq!(
        read_password_from(&mut input, false).expect("lecture"),
        "correct horse battery"
    );
}

#[test]
fn read_password_strips_line_endings_and_keeps_inner_spaces() {
    let mut input = Cursor::new(b"  mot de passe  \r\n".to_vec());
    assert_eq!(
        read_password_from(&mut input, false).expect("lecture"),
        "  mot de passe  "
    );
}

#[test]
fn read_password_accepts_empty_input() {
    let mut input = Cursor::new(Vec::new());
    assert_eq!(read_password_from(&mut input, false).expect("lecture"), "");
}
