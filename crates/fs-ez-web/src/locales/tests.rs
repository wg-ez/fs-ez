use super::*;

#[test]
fn loads_both_languages_and_falls_back_to_key() {
    let locales = Locales::load().expect("locales");
    assert!(locales.en.is_object());
    assert!(locales.fr.is_object());
    assert_eq!(locales.t("en", "meta.lang"), "en");
    assert_eq!(locales.t("fr", "meta.lang"), "fr");
    assert_eq!(locales.t("en", "absente.cle"), "absente.cle");
    assert!(locales.has_lang("fr"));
    assert!(!locales.has_lang("de"));
}

#[test]
fn french_covers_every_english_key_and_nothing_more() {
    let locales = Locales::load().expect("locales");
    assert!(
        locales.missing_keys().is_empty(),
        "clés fr manquantes : {:?}",
        locales.missing_keys()
    );
    assert_eq!(locales.keys("en"), locales.keys("fr"));
}
