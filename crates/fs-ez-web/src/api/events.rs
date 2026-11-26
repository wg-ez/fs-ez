//! `GET /api/events` : flux SSE des changements, session ou jeton Bearer.

use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderValue, header};
use axum::response::sse::{KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;

use crate::backend::Backend;
use crate::error::WebError;
use crate::events;
use crate::state::AppState;

pub async fn stream<B: Backend>(State(state): State<AppState<B>>) -> Result<Response, WebError> {
    // Abonnement avant la trame hello : aucun événement émis ensuite n'est perdu.
    let rx = state.events.subscribe();
    let hello = events::into_sse(events::hello_frame(state.backend.version()));
    let tail = futures_util::stream::unfold(rx, |mut rx| async move {
        events::received(rx.recv().await).map(|frame| {
            (
                Ok::<_, std::convert::Infallible>(events::into_sse(frame)),
                rx,
            )
        })
    });
    let stream = futures_util::stream::once(async move { Ok(hello) }).chain(tail);
    let mut response = Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
        .headers_mut()
        .insert("x-accel-buffering", HeaderValue::from_static("no"));
    Ok(response)
}
