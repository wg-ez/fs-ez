//! Préférences d'affichage du shell : langue, thème et query conservée.

use std::collections::HashMap;
use std::sync::Arc;

use axum::http::{HeaderMap, Uri, header::ACCEPT_LANGUAGE};
use axum_extra::extract::cookie::CookieJar;
use fs_ez_core::Settings;
use serde::Deserialize;

use crate::ctx::Base;
use crate::locales::Locales;
use crate::prefs::{self, Prefs};

#[derive(Debug, Default, Deserialize)]
pub struct UiQuery {
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
}

impl UiQuery {
    pub fn as_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for (key, value) in [("theme", &self.theme), ("lang", &self.lang)] {
            if let Some(value) = value {
                map.insert(key.to_string(), value.clone());
            }
        }
        map
    }
}

pub fn resolve_lang(headers: &HeaderMap, params: &UiQuery, settings: Option<&Settings>) -> String {
    resolve(headers, params, settings).lang
}

pub fn resolve_theme(headers: &HeaderMap, params: &UiQuery, settings: Option<&Settings>) -> String {
    resolve(headers, params, settings).theme
}

pub fn resolve(headers: &HeaderMap, params: &UiQuery, settings: Option<&Settings>) -> Prefs {
    let fallback = Settings::default();
    let cookies = CookieJar::from_headers(headers);
    prefs::resolve_with_language(
        &params.as_map(),
        &cookies,
        settings.unwrap_or(&fallback),
        accept_language(headers).as_deref(),
    )
}

fn accept_language(headers: &HeaderMap) -> Option<String> {
    headers
        .get(ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

/// Paramètres autres que `theme`/`lang`, suffixés par `&` s'ils existent.
pub fn preserved_query(uri: &Uri) -> String {
    let Some(query) = uri.query() else {
        return String::new();
    };
    let kept: Vec<&str> = query
        .split('&')
        .filter(|pair| {
            let key = pair.split_once('=').map_or(*pair, |(key, _)| key);
            key != "theme" && key != "lang" && !pair.is_empty()
        })
        .collect();
    if kept.is_empty() {
        String::new()
    } else {
        format!("{}&", kept.join("&"))
    }
}

#[allow(clippy::too_many_arguments)]
pub fn base(
    locales: Arc<Locales>,
    lang: &str,
    theme: &str,
    username: String,
    version: &'static str,
    active: String,
) -> Base {
    Base::new(
        Arc::clone(&locales),
        locales.t(lang, "app.title"),
        lang.to_string(),
        theme.to_string(),
        username,
        version,
        active,
    )
}
