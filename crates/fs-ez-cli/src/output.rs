//! Sortie humain/JSON ; n'imprime jamais directement.

use serde_json::{Map, Value};

pub struct Output {
    pub human: String,
    pub json: Value,
}

impl Output {
    pub fn new(human: impl Into<String>, json: Value) -> Self {
        Self {
            human: human.into(),
            json: with_schema(json),
        }
    }

    pub fn render(&self, json: bool) -> String {
        if !json {
            return self.human.clone();
        }
        match serde_json::to_string(&self.json) {
            Ok(text) => format!("{text}\n"),
            Err(_) => "{\"schema\":1}\n".to_string(),
        }
    }

    pub fn schema_v1() -> Value {
        Value::Object(schema_object())
    }
}

fn schema_object() -> Map<String, Value> {
    let mut object = Map::new();
    object.insert("schema".to_string(), Value::from(1));
    object
}

fn with_schema(json: Value) -> Value {
    match json {
        Value::Object(mut object) => {
            object.entry("schema").or_insert(Value::from(1));
            Value::Object(object)
        }
        other => {
            let mut object = schema_object();
            object.insert("data".to_string(), other);
            Value::Object(object)
        }
    }
}

#[cfg(test)]
mod tests;
