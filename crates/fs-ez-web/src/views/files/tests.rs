use super::*;
use time::OffsetDateTime;
use time::macros::format_description;

#[test]
fn sizes_use_binary_units() {
    assert_eq!(format_size(0), "0 o");
    assert_eq!(format_size(1023), "1023 o");
    assert_eq!(format_size(1024), "1.0 Kio");
    assert_eq!(format_size(1536), "1.5 Kio");
    assert_eq!(format_size(10 * 1024 * 1024), "10 Mio");
    assert_eq!(format_size(3 * 1024 * 1024 * 1024), "3.0 Gio");
}

#[test]
fn dates_use_the_local_offset_at_minute_precision() {
    let at = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp");
    let expected = fs_ez_core::datetime::to_local(at)
        .format(format_description!("[year]-[month]-[day] [hour]:[minute]"))
        .expect("format");
    assert_eq!(format_mtime(at), expected);
}
