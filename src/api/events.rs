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
            let (event_type, data) = match &event {
                SyncEvent::NewNotifications { count } => {
                    let payload = NewNotificationsData { count: *count };
                    (
                        "notifications:new",
                        serde_json::to_string(&payload)
                            .expect("serialization of NewNotificationsData cannot fail"),
                    )
                }
                SyncEvent::SyncStatus { status } => {
                    let payload = SyncStatusData {
                        status: status.clone(),
                    };
                    (
                        "sync:status",
                        serde_json::to_string(&payload)
                            .expect("serialization of SyncStatusData cannot fail"),
                    )
                }
                SyncEvent::PrTeamsUpdated(data) => {
                    let payload = serde_json::to_string(data)
                        .expect("serialization of PrTeamsUpdatedData cannot fail");
                    ("pr:teams_updated", payload)
                }
                SyncEvent::PrInfoUpdated(data) => {
                    let payload = serde_json::to_string(data)
                        .expect("serialization of PrInfoUpdatedData cannot fail");
                    ("pr:info_updated", payload)
                }
                SyncEvent::GithubSyncError(data) => {
                    let payload = serde_json::to_string(data)
                        .expect("serialization of GithubSyncErrorData cannot fail");
                    ("github:sync_error", payload)
                }
            };
            Some(Ok(Event::default().event(event_type).data(data)))
        }
        Err(_) => {
            // Lagged — skip missed messages
            eprintln!("SSE client lagged, skipping missed events");
            None
        }
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(30))
            .text("keep-alive"),
    )
}
