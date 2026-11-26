use super::*;

#[test]
fn usage_and_io_displays() {
    assert_eq!(
        CliError::Usage("bad".to_string()).to_string(),
        "usage error: bad"
    );
    assert_eq!(
        CliError::Io("disk".to_string()).to_string(),
        "io error: disk"
    );
}
