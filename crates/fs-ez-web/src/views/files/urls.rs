//! Construction des URLs de navigation et de rafraîchissement.

use fs_ez_core::ViewId;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};

use crate::query_params::{ValidatedFiles, filter_json_token};

fn push_tags(pairs: &mut Vec<(&str, String)>, key: &'static str, tags: &[String]) {
    pairs.extend(tags.iter().map(|tag| (key, tag.clone())));
}

fn encode(base: &str, pairs: Vec<(&str, String)>) -> String {
    let mut url = base.to_string();
    for (index, (key, value)) in pairs.into_iter().enumerate() {
        url.push(if index == 0 { '?' } else { '&' });
        url.push_str(key);
        url.push('=');
        url.push_str(&utf8_percent_encode(&value, NON_ALPHANUMERIC).to_string());
    }
    url
}

/// Paramètres d'URL d'une liste, jetons calculés une fois puis réutilisés.
#[derive(Clone)]
pub(super) struct ListQuery<'a> {
    q: Option<&'a str>,
    filter: Option<&'static str>,
    filter_json: Option<String>,
    tags: &'a [String],
    exclude_tags: &'a [String],
    sort: String,
    view: Option<ViewId>,
    folder: Option<String>,
}

impl<'a> ListQuery<'a> {
    pub(super) fn new(params: &'a ValidatedFiles, sort: String) -> Self {
        Self {
            q: params.q.as_deref(),
            filter: params.filter.map(|filter| filter.token()),
            filter_json: params.filter_expr.as_ref().and_then(filter_json_token),
            tags: &params.tags,
            exclude_tags: &params.exclude_tags,
            sort,
            view: None,
            folder: params.virtual_folder_token(),
        }
    }

    /// Liens de rafraîchissement : la vue est portée par l'URL de la page.
    pub(super) fn with_view(&self, view: Option<ViewId>) -> Self {
        Self {
            view,
            ..self.clone()
        }
    }

    pub(super) fn set_sort(&mut self, sort: String) {
        self.sort = sort;
    }
}

pub(super) fn files_url(path: &str, query: &ListQuery<'_>) -> String {
    let mut pairs: Vec<(&str, String)> = Vec::new();
    if let Some(q) = query.q.filter(|value| !value.is_empty()) {
        pairs.push(("q", q.to_string()));
    }
    if let Some(filter) = query.filter {
        pairs.push(("filter", filter.to_string()));
    }
    if let Some(filter_json) = &query.filter_json {
        pairs.push(("filter_json", filter_json.clone()));
    }
    push_tags(&mut pairs, "tag", query.tags);
    push_tags(&mut pairs, "not_tag", query.exclude_tags);
    pairs.push(("sort", query.sort.clone()));
    if let Some(view) = query.view {
        pairs.push(("view", view.to_string()));
    }
    if let Some(folder) = &query.folder {
        pairs.push(("vf", folder.clone()));
    }
    encode(path, pairs)
}
