use std::sync::Arc;

use fs_ez_core::{Settings, ViewId};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};

use super::choices::{Choice, choices_from, number_choices, sort_options};
use crate::locales::Locales;
use crate::query_params::sort_token;

pub struct Base {
    pub title: String,
    pub lang: String,
    pub theme: String,
    pub username: String,
    pub version: &'static str,
    pub asset_version: &'static str,
    pub active: String,
    pub collapsed: bool,
    pub recent_days: i64,
    pub recent_file_days: i64,
    pub page_size: usize,
    pub default_sort: String,
    pub backup_days: i64,
    pub backup_hour: i64,
    /// Tags actifs de la recherche globale, dans l'ordre des filtres d'URL.
    pub search_tags: Vec<String>,
    /// Tags exclus de la recherche globale, dans l'ordre des filtres d'URL.
    pub search_excluded_tags: Vec<String>,
    /// Paramètres de liste conservés en champs cachés par la recherche globale.
    pub search_fields: Vec<SearchField>,
    locales: Arc<Locales>,
}

pub struct SearchField {
    pub name: String,
    pub value: String,
}

/// Fenêtres en jours partagées par les menus d'apparence.
const DAY_PRESETS: [i64; 9] = [1, 3, 7, 14, 30, 60, 90, 180, 365];

impl Base {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        locales: Arc<Locales>,
        title: String,
        lang: String,
        theme: String,
        username: String,
        version: &'static str,
        active: String,
    ) -> Self {
        let mut base = Self {
            title,
            lang,
            theme,
            username,
            version,
            asset_version: crate::assets::ASSET_VERSION,
            active,
            collapsed: false,
            recent_days: 0,
            recent_file_days: 0,
            page_size: 0,
            default_sort: String::new(),
            backup_days: 0,
            backup_hour: 0,
            search_tags: Vec::new(),
            search_excluded_tags: Vec::new(),
            search_fields: Vec::new(),
            locales,
        };
        base.set_settings(&Settings::default());
        base
    }

    /// Préférences de liste du menu d'apparence, appliquées avant le premier rendu.
    pub fn set_settings(&mut self, settings: &Settings) {
        self.recent_days = settings.recent_days;
        self.recent_file_days = settings.recent_file_days;
        self.page_size = settings.page_size;
        self.default_sort = sort_token(settings.default_sort);
        self.backup_days = settings.backup_days;
        self.backup_hour = settings.backup_hour;
    }

    pub fn is_search_tag(&self, name: &str) -> bool {
        self.search_tags.iter().any(|tag| tag == name)
    }

    pub fn is_search_excluded(&self, name: &str) -> bool {
        self.search_excluded_tags.iter().any(|tag| tag == name)
    }

    /// Filtre de tags actif, inclusion ou blacklist, pour le badge et l'état « tous ».
    pub fn has_search_tags(&self) -> bool {
        !self.search_tags.is_empty() || !self.search_excluded_tags.is_empty()
    }

    pub fn search_tag_count(&self) -> usize {
        self.search_tags.len() + self.search_excluded_tags.len()
    }

    /// Vue de la sidebar correspondant à la page courante.
    pub fn is_active_view(&self, id: &ViewId) -> bool {
        self.active == format!("view:{id}")
    }

    /// Tag de la sidebar filtrant la page courante.
    pub fn is_active_tag(&self, name: &str) -> bool {
        self.active == "tags" && self.is_search_tag(name)
    }

    /// Lien de sidebar conservant le tri courant, le reste de l'URL reste celui du lien.
    pub fn sort_href(&self, href: impl std::fmt::Display) -> String {
        let href = href.to_string();
        let Some(sort) = self.search_fields.iter().find(|field| field.name == "sort") else {
            return href;
        };
        let separator = if href.contains('?') { '&' } else { '?' };
        format!(
            "{href}{separator}sort={}",
            utf8_percent_encode(&sort.value, NON_ALPHANUMERIC)
        )
    }

    /// Jamais de panique : une clé absente s'affiche telle quelle.
    pub fn t(&self, key: &str) -> String {
        self.locales.t(&self.lang, key)
    }

    /// Palette localisée des contrôles de couleur.
    pub fn color_label(&self, token: &str) -> String {
        self.t(&format!("color.{token}"))
    }

    /// Tri proposés par l'éditeur de vue et le menu d'apparence.
    pub fn view_sort_choices(&self, current: &str) -> Vec<Choice> {
        sort_options(&self.locales, &self.lang, current)
    }

    /// Fenêtres proposées par le menu d'apparence, la valeur courante est toujours présente.
    pub fn recent_day_choices(&self) -> Vec<Choice> {
        self.day_choices(self.recent_days)
    }

    /// Fenêtres des fichiers récents, la valeur courante est toujours présente.
    pub fn recent_file_day_choices(&self) -> Vec<Choice> {
        self.day_choices(self.recent_file_days)
    }

    /// Tailles de page proposées par le menu d'apparence, la valeur courante est toujours présente.
    pub fn page_size_choices(&self) -> Vec<Choice> {
        let unit = self.t("shell.files");
        let current = i64::try_from(self.page_size).unwrap_or(i64::MAX);
        number_choices(&[25, 50, 100, 200, 250, 500], current, &unit)
    }

    /// Fenêtres de conservation des sauvegardes, la valeur courante est toujours présente.
    pub fn backup_day_choices(&self) -> Vec<Choice> {
        self.day_choices(self.backup_days)
    }

    /// Heures de la sauvegarde quotidienne, la valeur courante est toujours présente.
    pub fn backup_hour_choices(&self) -> Vec<Choice> {
        choices_from(&(0..24).collect::<Vec<_>>(), self.backup_hour, |value| {
            format!("{value:02}:00")
        })
    }

    /// Options de fenêtres en jours, l'unité est localisée.
    fn day_choices(&self, current: i64) -> Vec<Choice> {
        let unit = self.t("shell.days");
        number_choices(&DAY_PRESETS, current, &unit)
    }
}
