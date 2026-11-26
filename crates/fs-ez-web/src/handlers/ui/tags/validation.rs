//! Validation des entrées de tag et d'identifiant de groupe.

use fs_ez_core::GroupId;

use crate::backend::Backend;
use crate::error::WebError;
use crate::forms::parse_positive_id;
use crate::state::AppState;

/// Nom de tag nettoyé ou refus localisé, jamais de message de normalisation brut.
pub(crate) fn clean_tag_name(
    state: &AppState<impl Backend>,
    lang: &str,
    raw: &str,
) -> Result<String, WebError> {
    if raw.trim().is_empty() {
        return Err(WebError::BadRequest(
            state.locales.t(lang, "tags.tag_name_required"),
        ));
    }
    match fs_ez_core::normalize_tag_name(raw) {
        Ok((display, _)) => Ok(display),
        Err(_) => Err(WebError::BadRequest(
            state.locales.t(lang, "tags.name_invalid"),
        )),
    }
}

pub(crate) fn parse_group_id(raw: Option<&str>) -> Result<GroupId, WebError> {
    raw.and_then(parse_positive_id)
        .map(GroupId)
        .ok_or_else(|| WebError::BadRequest("invalid group id".to_string()))
}
