//! Tests du formatage et du parsing RFC3339.

use super::*;
use time::macros::datetime;

#[test]
fn parse_rfc3339_valide() {
    let value = parse_rfc3339("2023-11-14T22:13:20Z").expect("date valide");
    assert_eq!(value, datetime!(2023-11-14 22:13:20 UTC));
}

#[test]
fn parse_rfc3339_invalide() {
    assert!(parse_rfc3339("pas une date").is_err());
    assert!(parse_rfc3339("").is_err());
}

#[test]
fn conversion_locale_preserve_l_instant() {
    let value = datetime!(2023-11-14 23:13:20 +01:00);
    assert_eq!(to_local(value).unix_timestamp(), value.unix_timestamp());
}

#[test]
fn format_rfc3339_stable_en_utc() {
    assert_eq!(
        format_rfc3339(datetime!(2023-11-14 22:13:20 UTC)),
        "2023-11-14T22:13:20Z"
    );
    assert_eq!(
        format_rfc3339_utc(datetime!(2023-11-14 23:13:20 +01:00)),
        "2023-11-14T22:13:20Z"
    );
}
