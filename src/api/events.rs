use std::convert::Infallible;

use axum::Router;
use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::routing::get;
use futures::stream::Stream;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::models::{NewNotificationsData, SyncEvent, SyncStatusData};
use crate::server::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/events", get(get_events))
}

/// GET /api/events — SSE stream of sync events.
pub async fn get_events(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(event) => {
            let (event_type, payload) = match &event {
                SyncEvent::NewNotifications { count } => (
                    "notifications:new",
                    serde_json::to_string(&NewNotificationsData { count: *count }),
                ),
                SyncEvent::SyncStatus { status } => (
                    "sync:status",
                    serde_json::to_string(&SyncStatusData {
                        status: status.clone(),
                    }),
                ),
                SyncEvent::PrInfoUpdated(data) => ("pr:info_updated", serde_json::to_string(data)),
                SyncEvent::GithubSyncError(data) => {
                    ("github:sync_error", serde_json::to_string(data))
                }
            };
            match payload {
                Ok(data) => Some(Ok(Event::default().event(event_type).data(data))),
                // Serialization of these payloads can't fail in practice, but
                // a handler must not panic — drop the event and log instead.
                Err(e) => {
                    tracing::error!(error = %e, event_type, "failed to serialize SSE event");
                    None
                }
            }
        }
        Err(_) => {
            // Lagged: the channel overflowed and this client missed an unknown
            // set of events. Unlike a disconnect, no `open` fires client-side,
            // so dropping silently would leave the UI stale. Emit a
            // notifications:new so the client refetches everything it renders.
            tracing::warn!("SSE client lagged; asking it to refetch");
            Some(Ok(Event::default()
                .event("notifications:new")
                .data("{\"count\":0}")))
        }
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(30))
            .text("keep-alive"),
    )
}
