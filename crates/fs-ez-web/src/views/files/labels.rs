//! Libellés i18n de la liste de fichiers.

use crate::locales::Locales;

#[derive(Clone)]
pub struct FilesLabels {
    pub name: String,
    pub tags: String,
    pub size: String,
    pub date: String,
    pub resize: String,
    pub resize_hint: String,
    pub loading: String,
    pub empty: String,
    pub actions: String,
    pub rename: String,
    pub delete: String,
    pub rename_single: String,
    pub delete_error: String,
    pub drop_error: String,
}

impl FilesLabels {
    pub fn load(locales: &Locales, lang: &str) -> Self {
        let t = |key: &str| locales.t(lang, key);
        Self {
            name: t("files.column.name"),
            tags: t("files.column.tags"),
            size: t("files.column.size"),
            date: t("files.column.date"),
            resize: t("files.column.resize"),
            resize_hint: t("files.column.resize_hint"),
            loading: t("files.loading"),
            empty: t("files.empty"),
            actions: t("files.actions"),
            rename: t("files.rename"),
            delete: t("files.delete"),
            rename_single: t("files.rename_single"),
            delete_error: t("files.delete_error"),
            drop_error: t("toast.error"),
        }
    }
}
