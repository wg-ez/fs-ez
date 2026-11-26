use super::*;

#[test]
fn display_and_conversions_round_trip() {
    let id = FileId::from(42);
    assert_eq!(id.to_string(), "42");
    assert_eq!(i64::from(id), 42);
    assert_eq!(FileId(42).0, 42);
}

#[test]
fn serde_is_transparent() {
    let json = serde_json::to_string(&TagId(7)).expect("serialize");
    assert_eq!(json, "7");
    let back: TagId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, TagId(7));
}

#[test]
fn deserialize_rejects_non_positive_ids() {
    assert_eq!(
        serde_json::from_str::<ViewId>("1").expect("positif"),
        ViewId(1)
    );
    assert!(serde_json::from_str::<ViewId>("0").is_err());
    assert!(serde_json::from_str::<ViewId>("-1").is_err());
}

#[test]
fn serialize_view_id_stays_transparent() {
    assert_eq!(serde_json::to_string(&ViewId(7)).expect("serialize"), "7");
}
