use fs_ez_core::{Sort, SortDir, SortField};

use crate::locales::Locales;

/// Option de menu : jeton soumis, libellé localisé et état sélectionné.
pub struct Choice {
    pub token: String,
    pub label: String,
    pub selected: bool,
}

/// Tris proposés par les formulaires de vue et les réglages.
pub(super) fn sort_options(locales: &Locales, lang: &str, current: &str) -> Vec<Choice> {
    let mut options = Vec::new();
    for field in [
        SortField::Id,
        SortField::Name,
        SortField::Size,
        SortField::Mtime,
    ] {
        for dir in [SortDir::Asc, SortDir::Desc] {
            let field_key = match field {
                SortField::Id => "id",
                SortField::Name => "name",
                SortField::Size => "size",
                SortField::Mtime => "date",
            };
            let token = Sort { field, dir }.encode();
            options.push(Choice {
                selected: token == current,
                token,
                label: format!(
                    "{} ({})",
                    locales.t(lang, &format!("views.sort_fields.{field_key}")),
                    locales.t(
                        lang,
                        match dir {
                            SortDir::Asc => "views.dir.asc",
                            SortDir::Desc => "views.dir.desc",
                        }
                    )
                ),
            });
        }
    }
    options
}

/// Options triées, la valeur courante est ajoutée puis triée si absente.
pub(super) fn choices_from(
    values: &[i64],
    current: i64,
    label: impl Fn(i64) -> String,
) -> Vec<Choice> {
    let extra = (!values.contains(&current)).then_some(current);
    let mut ordered: Vec<&i64> = values.iter().chain(extra.iter()).collect();
    ordered.sort_unstable();
    ordered
        .into_iter()
        .map(|&value| Choice {
            token: value.to_string(),
            label: label(value),
            selected: value == current,
        })
        .collect()
}

/// Options numériques triées, l'unité et la sélection viennent du réglage courant.
pub(super) fn number_choices(presets: &[i64], current: i64, unit: &str) -> Vec<Choice> {
    choices_from(presets, current, |value| format!("{value} {unit}"))
}
