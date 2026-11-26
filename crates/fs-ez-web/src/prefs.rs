//! Préférences par requête : langue, thème, priorités figées.

use std::collections::HashMap;

use axum_extra::extract::cookie::{Cookie, CookieJar};
use fs_ez_core::{Language, Settings, Theme};
use time::Duration;

use crate::cookie_util;

pub const LANG_COOKIE: &str = "fs-ez_lang";
pub const THEME_COOKIE: &str = "fs-ez_theme";
/// Un an, préférences d'affichage non sensibles.
const MAX_AGE_SECONDS: i64 = 31_536_000;

pub struct Prefs {
    pub lang: String,
    pub theme: String,
}

/// Priorité langue : `?lang=` > cookie > `Accept-Language` > réglage > `en`.
pub fn resolve_with_language(
    query: &HashMap<String, String>,
    cookies: &CookieJar,
    settings: &Settings,
    accept_language: Option<&str>,
) -> Prefs {
    let lang = query
        .get("lang")
        .and_then(|raw| Language::parse(raw))
        .or_else(|| {
            cookies
                .get(LANG_COOKIE)
                .and_then(|c| Language::parse(c.value()))
        })
        .or_else(|| accept_language.and_then(preferred))
        .unwrap_or(settings.language)
        .as_token()
        .to_string();
    let theme = query
        .get("theme")
        .and_then(|raw| Theme::parse(raw))
        .or_else(|| {
            cookies
                .get(THEME_COOKIE)
                .and_then(|c| Theme::parse(c.value()))
        })
        .unwrap_or(settings.theme)
        .as_token()
        .to_string();
    Prefs { lang, theme }
}

/// Premier `en`/`fr` de l'en-tête, préfixe régional ignoré (`fr-CA` → `fr`).
pub fn preferred(raw: &str) -> Option<Language> {
    raw.split(',').find_map(|part| {
        let tag = part
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let base = tag.split('-').next().unwrap_or_default();
        Language::parse(base)
    })
}

/// Cookie non `HttpOnly` : le thème et la langue ne sont pas des secrets.
pub fn lang_cookie(lang: &str, secure: bool) -> Cookie<'static> {
    prefs_cookie(LANG_COOKIE, lang, secure)
}

pub fn theme_cookie(theme: &str, secure: bool) -> Cookie<'static> {
    prefs_cookie(THEME_COOKIE, theme, secure)
}

fn prefs_cookie(name: &'static str, value: &str, secure: bool) -> Cookie<'static> {
    cookie_util::cookie(
        name,
        value,
        false,
        Duration::seconds(MAX_AGE_SECONDS),
        secure,
    )
}

/// Paires `clé=valeur` décodées ; une clé vide est ignorée, la dernière valeur gagne.
pub fn query_map(raw: Option<&str>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (key, value) in form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
        if !key.is_empty() {
            map.insert(key.into_owned(), value.into_owned());
        }
    }
    map
}

#[cfg(test)]
mod tests;
