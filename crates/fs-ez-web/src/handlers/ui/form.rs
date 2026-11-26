//! Décodage et validation des formulaires UI, extracteur `UiForm`.

use axum::body::Bytes;
use axum::extract::{FromRequest, Request};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use fs_ez_app::{AppError, parse_tag_names};
use fs_ez_core::{TagColor, normalize_group_name};

use crate::backend::Backend;
use crate::error::WebError;
use crate::forms::parse_optional_color;
use crate::handlers::ui::chrome::UiQuery;
use crate::handlers::ui::pages::load_lang;
use crate::state::AppState;

pub struct UiForm(pub Vec<(String, String)>);

impl<S> FromRequest<S> for UiForm
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = match Bytes::from_request(req, state).await {
            Ok(bytes) => bytes,
            Err(error) if error.status() == StatusCode::PAYLOAD_TOO_LARGE => {
                return Err(WebError::PayloadTooLarge.into_response());
            }
            Err(_) => {
                return Err(WebError::BadRequest("invalid form body".to_string()).into_response());
            }
        };
        parse_form(&bytes)
            .map(UiForm)
            .map_err(|error| error.into_response())
    }
}

pub(crate) fn parse_form(body: &[u8]) -> Result<Vec<(String, String)>, WebError> {
    let text = std::str::from_utf8(body)
        .map_err(|_| WebError::BadRequest("form body is not UTF-8".to_string()))?;
    let mut pairs = Vec::new();
    for pair in text.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        pairs.push((decode(key)?, decode(value)?));
    }
    Ok(pairs)
}

fn decode(raw: &str) -> Result<String, WebError> {
    let with_spaces = raw.replace('+', " ");
    percent_encoding::percent_decode_str(&with_spaces)
        .decode_utf8()
        .map(|value| value.into_owned())
        .map_err(|_| WebError::BadRequest("invalid percent-encoding".to_string()))
}

pub(crate) fn first_value<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(candidate, _)| candidate == key)
        .map(|(_, value)| value.as_str())
}

/// Découpe sur virgules, `parse_tag_names` normalise et déduplique.
pub(crate) fn parse_tags(raw: &str) -> Result<Vec<String>, AppError> {
    let pieces: Vec<String> = raw
        .split(',')
        .map(|piece| piece.trim().to_string())
        .collect();
    parse_tag_names(&pieces)
}

/// Refus de formulaire à re-rendre, ou réponse brute (`load_lang`) sans re-rendu.
pub(crate) enum GroupFormError {
    Form(WebError),
    Raw(Box<Response>),
}

/// Nom validé après `load_lang`, une erreur de langue reste une réponse brute.
pub(crate) async fn parse_group_name<B: Backend>(
    state: &AppState<B>,
    ui: &UiQuery,
    headers: &HeaderMap,
    raw: &str,
    required_key: &str,
) -> Result<String, GroupFormError> {
    let lang = load_lang(state, headers, ui)
        .await
        .map_err(|error| GroupFormError::Raw(Box::new(error.into_response())))?;
    // Distingue le nom vide des autres refus pour garder le message i18n.
    if raw.trim().is_empty() {
        return Err(GroupFormError::Form(WebError::BadRequest(
            state.locales.t(&lang, required_key),
        )));
    }
    normalize_group_name(raw).ok_or_else(|| {
        GroupFormError::Form(WebError::BadRequest(
            state.locales.t(&lang, "tags.name_invalid"),
        ))
    })
}

/// Ordre couleur → langue → nom : une couleur invalide n'exige pas la langue.
pub(crate) async fn parse_group_draft<B: Backend>(
    state: &AppState<B>,
    ui: &UiQuery,
    headers: &HeaderMap,
    pairs: &[(String, String)],
    required_key: &str,
) -> Result<(String, Option<TagColor>), GroupFormError> {
    let color = parse_optional_color(first_value(pairs, "color")).map_err(GroupFormError::Form)?;
    let name = parse_group_name(
        state,
        ui,
        headers,
        first_value(pairs, "name").unwrap_or_default(),
        required_key,
    )
    .await?;
    Ok((name, color))
}
