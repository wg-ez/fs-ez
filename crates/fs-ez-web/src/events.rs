//! Bus d'événements SSE : publication, veille `data_version` et encodage des trames.

use std::time::Duration;

use axum::response::sse::Event as SseEvent;
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;

use crate::api::errors::SCHEMA_VERSION;
use crate::backend::Backend;
use crate::state::{AppState, Event};

/// Trame SSE pure, séparée d'axum pour tester le mapping sans flux infini.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub name: &'static str,
    pub data: String,
}

/// Publie sans erreur : l'absence d'abonné est un cas nominal.
pub fn publish<B: Backend>(state: &AppState<B>, event: Event) {
    let _ = state.events.send(event);
}

pub fn hello_frame(version: &str) -> Frame {
    Frame {
        name: "hello",
        data: json!({
            "schema": SCHEMA_VERSION,
            "version": version,
        })
        .to_string(),
    }
}

pub fn frame(event: &Event) -> Frame {
    match event {
        Event::DataChanged => simple("data-changed"),
        Event::InboxChanged => simple("inbox-changed"),
        Event::ImportFinished {
            imported,
            duplicates,
        } => Frame {
            name: "import-finished",
            data: json!({
                "schema": SCHEMA_VERSION,
                "imported": imported,
                "duplicates": duplicates,
            })
            .to_string(),
        },
        Event::Resync => simple("resync"),
    }
}

/// `Lagged` devient `resync`, `Closed` termine le flux.
pub fn received(result: Result<Event, RecvError>) -> Option<Frame> {
    match result {
        Ok(event) => Some(frame(&event)),
        Err(RecvError::Lagged(_)) => Some(frame(&Event::Resync)),
        Err(RecvError::Closed) => None,
    }
}

pub fn into_sse(frame: Frame) -> SseEvent {
    SseEvent::default().event(frame.name).data(frame.data)
}

/// Poll léger : `PRAGMA data_version` ne bouge que sur commit d'une autre connexion.
pub fn spawn_data_watcher<B: Backend>(state: AppState<B>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut last: Option<i64> = None;
        let mut logged = false;
        loop {
            interval.tick().await;
            let current = match crate::backend::blocking(state.backend.app(), |app| {
                Ok(app.store.data_version()?)
            })
            .await
            {
                Ok(current) => current,
                Err(error) => {
                    if !logged {
                        tracing::warn!(%error, "data version poll failed");
                        logged = true;
                    }
                    continue;
                }
            };
            if let Some(previous) = last
                && previous != current
            {
                publish(&state, Event::DataChanged);
            }
            last = Some(current);
            logged = false;
        }
    })
}

fn simple(name: &'static str) -> Frame {
    Frame {
        name,
        data: json!({ "schema": SCHEMA_VERSION }).to_string(),
    }
}

#[cfg(test)]
mod tests;
