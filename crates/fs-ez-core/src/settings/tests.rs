use super::*;

#[test]
fn defaults_match_spec() {
    let settings = Settings::default();
    assert_eq!(settings.recent_days, 7);
    assert_eq!(settings.recent_file_days, 7);
    assert_eq!(settings.default_sort, Sort::default());
    assert_eq!(settings.page_size, 100);
    assert!(!settings.sidebar_collapsed);
    assert!(settings.open_groups.is_empty());
    assert_eq!(settings.theme, Theme::Light);
    assert_eq!(settings.language, Language::En);
    assert_eq!(settings.files_dir, "files");
    assert_eq!(settings.backup_days, 7);
    assert_eq!(settings.backup_hour, 3);
}

#[test]
fn pairs_round_trip() {
    let mut settings = Settings::default();
    settings.set("ui.recent_days", "30").expect("jours");
    settings
        .set("ui.recent_file_days", "14")
        .expect("jours fichiers");
    settings.set("ui.sort_field", "mtime").expect("tri");
    settings.set("ui.sort_dir", "desc").expect("sens");
    settings.set("ui.page_size", "250").expect("taille");
    settings.set("ui.sidebar_collapsed", "1").expect("repli");
    settings.set("ui.open_groups", "3,12").expect("groupes");
    settings.set("ui.theme", "dark").expect("thème");
    settings.set("ui.language", "fr").expect("langue");
    settings.set("io.files_dir", "in/black").expect("files");
    settings.set("io.backup_days", "3").expect("sauvegardes");
    settings.set("io.backup_hour", "4").expect("heure");

    let pairs = settings.to_pairs();
    let back = Settings::from_pairs(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    assert_eq!(back, settings);
    assert_eq!(settings.get("ui.open_groups").as_deref(), Some("3,12"));
    assert_eq!(settings.get("io.backup_days").as_deref(), Some("3"));
    assert_eq!(settings.get("io.backup_hour").as_deref(), Some("4"));
    assert_eq!(settings.get("missing"), None);
}

#[test]
fn invalid_pair_values_are_ignored() {
    let settings = Settings::from_pairs(
        [
            ("ui.recent_days", "0"),
            ("ui.page_size", "501"),
            ("ui.theme", "neon"),
            ("ui.language", "de"),
            ("ui.open_groups", "1,x"),
            ("unknown.key", "value"),
        ]
        .into_iter(),
    );
    assert_eq!(settings, Settings::default());
}

#[test]
fn rejects_bad_values_explicitly() {
    let mut settings = Settings::default();
    assert!(settings.set("ui.recent_days", "366").is_err());
    assert!(settings.set("ui.recent_file_days", "0").is_err());
    assert!(settings.set("ui.page_size", "0").is_err());
    assert!(settings.set("ui.sidebar_collapsed", "true").is_err());
    assert!(settings.set("ui.sort_field", "date").is_err());
    assert!(settings.set("io.files_dir", "/abs").is_err());
    assert!(settings.set("io.backup_days", "0").is_err());
    assert!(settings.set("io.backup_days", "366").is_err());
    assert!(settings.set("io.backup_hour", "24").is_err());
    assert!(settings.set("io.backup_hour", "-1").is_err());
    assert!(settings.set("nope", "x").is_err());
    assert_eq!(settings, Settings::default());
}

#[test]
fn theme_language_tokens() {
    for theme in [Theme::Light, Theme::Dark, Theme::System] {
        assert_eq!(Theme::parse(theme.as_token()), Some(theme));
    }
    for language in [Language::En, Language::Fr] {
        assert_eq!(Language::parse(language.as_token()), Some(language));
    }
    assert_eq!(Theme::parse("Light"), None);
    assert_eq!(Language::parse("EN"), None);
}
