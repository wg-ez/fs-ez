use axum_extra::extract::cookie::SameSite;

use super::*;

fn jar(pairs: &[(&str, &str)]) -> CookieJar {
    let mut jar = CookieJar::new();
    for (name, value) in pairs {
        jar = jar.add(Cookie::new((*name).to_string(), (*value).to_string()));
    }
    jar
}

#[test]
fn query_wins_over_cookie_and_settings() {
    let mut query = HashMap::new();
    query.insert("lang".to_string(), "fr".to_string());
    query.insert("theme".to_string(), "dark".to_string());
    let settings = Settings {
        language: Language::En,
        theme: Theme::Light,
        ..Settings::default()
    };
    let prefs = resolve_with_language(
        &query,
        &jar(&[(LANG_COOKIE, "en"), (THEME_COOKIE, "light")]),
        &settings,
        None,
    );
    assert_eq!(prefs.lang, "fr");
    assert_eq!(prefs.theme, "dark");
}

#[test]
fn cookie_wins_over_settings_and_accept_language() {
    let settings = Settings::default();
    let prefs = resolve_with_language(
        &HashMap::new(),
        &jar(&[(LANG_COOKIE, "en"), (THEME_COOKIE, "system")]),
        &settings,
        Some("fr-FR,fr;q=0.9"),
    );
    assert_eq!(prefs.lang, "en");
    assert_eq!(prefs.theme, "system");
}

#[test]
fn accept_language_then_settings_then_default() {
    let settings = Settings {
        language: Language::Fr,
        ..Settings::default()
    };
    let prefs = resolve_with_language(
        &HashMap::new(),
        &CookieJar::new(),
        &settings,
        Some("de-DE,en;q=0.8"),
    );
    assert_eq!(prefs.lang, "en");

    let prefs = resolve_with_language(&HashMap::new(), &CookieJar::new(), &settings, Some("de-DE"));
    assert_eq!(prefs.lang, "fr");
}

#[test]
fn invalid_query_values_are_ignored() {
    let mut query = HashMap::new();
    query.insert("lang".to_string(), "de".to_string());
    query.insert("theme".to_string(), "neon".to_string());
    let settings = Settings::default();
    let prefs = resolve_with_language(&query, &CookieJar::new(), &settings, None);
    assert_eq!(prefs.lang, "en");
    assert_eq!(prefs.theme, "light");
}

#[test]
fn cookies_are_not_http_only_and_lax() {
    let cookie = lang_cookie("fr", true);
    assert_eq!(cookie.name(), LANG_COOKIE);
    assert_eq!(cookie.path(), Some("/"));
    assert_eq!(cookie.http_only(), Some(false));
    assert_eq!(cookie.same_site(), Some(SameSite::Lax));
    assert_eq!(cookie.secure(), Some(true));
    assert_eq!(cookie.max_age(), Some(Duration::seconds(MAX_AGE_SECONDS)));
    assert_eq!(theme_cookie("dark", false).secure(), Some(false));
}

#[test]
fn query_map_decodes_and_skips_empty_keys() {
    let map = query_map(Some("lang=fr&theme=dark&empty=&=x"));
    assert_eq!(map.get("lang").map(String::as_str), Some("fr"));
    assert_eq!(map.get("theme").map(String::as_str), Some("dark"));
    assert_eq!(map.get("empty").map(String::as_str), Some(""));
    assert!(!map.contains_key(""));
}

#[test]
fn query_map_empty_input_yields_empty_map() {
    assert!(query_map(None).is_empty());
    assert!(query_map(Some("")).is_empty());
    assert!(query_map(Some("&&")).is_empty());
}

#[test]
fn query_map_decodes_plus_and_percent_utf8() {
    let map = query_map(Some("theme=dark+mode&lang=fr%2DCA&label=caf%C3%A9"));
    assert_eq!(map.get("theme").map(String::as_str), Some("dark mode"));
    assert_eq!(map.get("lang").map(String::as_str), Some("fr-CA"));
    assert_eq!(map.get("label").map(String::as_str), Some("café"));
}

#[test]
fn query_map_duplicate_key_keeps_last_value() {
    let map = query_map(Some("lang=en&lang=fr&theme=dark&theme=light"));
    assert_eq!(map.get("lang").map(String::as_str), Some("fr"));
    assert_eq!(map.get("theme").map(String::as_str), Some("light"));
}
