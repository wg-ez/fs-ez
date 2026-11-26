//! Locales embarquées EN/FR, chargées une fois au démarrage.

use fs_ez_core::Language;
use serde_json::Value;

use crate::error::WebError;

pub struct Locales {
    pub en: Value,
    pub fr: Value,
}

impl Locales {
    pub fn load() -> Result<Self, WebError> {
        let en = parse("en", include_str!("../../../locales/en.json"))?;
        let fr = parse("fr", include_str!("../../../locales/fr.json"))?;
        Ok(Self { en, fr })
    }

    /// Clé pointée (`nav.inbox`) : clé brute retournée si absente.
    pub fn t(&self, lang: &str, key: &str) -> String {
        let tree = if lang == "fr" { &self.fr } else { &self.en };
        let mut current = tree;
        for part in key.split('.') {
            match current.get(part) {
                Some(next) => current = next,
                None => return key.to_string(),
            }
        }
        current
            .as_str()
            .map_or_else(|| key.to_string(), |value| value.to_string())
    }

    pub fn has_lang(&self, lang: &str) -> bool {
        Language::parse(lang).is_some()
    }

    /// Clés feuilles en notation pointée, triées.
    pub fn keys(&self, lang: &str) -> Vec<String> {
        let tree = if lang == "fr" { &self.fr } else { &self.en };
        let mut collected = Vec::new();
        collect_keys(tree, "", &mut collected);
        collected.sort();
        collected
    }

    /// Clés de `en` absentes de `fr` : le test de parité exige l'ensemble vide.
    pub fn missing_keys(&self) -> Vec<String> {
        let fr = self.keys("fr");
        self.keys("en")
            .into_iter()
            .filter(|key| !fr.contains(key))
            .collect()
    }
}

fn collect_keys(value: &Value, prefix: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                collect_keys(child, &path, out);
            }
        }
        Value::String(_) => out.push(prefix.to_string()),
        _ => {}
    }
}

fn parse(lang: &str, text: &str) -> Result<Value, WebError> {
    serde_json::from_str(text)
        .map_err(|error| WebError::Internal(format!("locales/{lang}.json: {error}")))
}

#[cfg(test)]
mod tests;
