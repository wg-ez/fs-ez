use super::*;

#[test]
fn schema_v1_is_a_versioned_object() {
    assert_eq!(Output::schema_v1(), serde_json::json!({"schema": 1}));
}

#[test]
fn render_human_or_valid_json() {
    let mut payload = Output::schema_v1();
    if let Value::Object(object) = &mut payload {
        object.insert("count".to_string(), Value::from(3));
    }
    let output = Output::new("3 fichiers", payload);
    assert_eq!(output.render(false), "3 fichiers");
    let text = output.render(true);
    assert!(text.ends_with('\n'));
    let value: Value = serde_json::from_str(&text).expect("json valide");
    assert_eq!(value["schema"], Value::from(1));
    assert_eq!(value["count"], Value::from(3));
}

#[test]
fn new_adds_schema_to_objects_and_wraps_scalars() {
    let object = Output::new("x", serde_json::json!({"a": 1}));
    assert_eq!(object.json["schema"], Value::from(1));
    let scalar = Output::new("x", Value::from("raw"));
    assert_eq!(scalar.json["schema"], Value::from(1));
    assert_eq!(scalar.json["data"], Value::from("raw"));
}
