use std::sync::atomic::Ordering;

use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;

use crate::api::AppError;
use crate::db::queries::{self, NotificationRow};
use crate::github::sync::sync_notifications;
use crate::server::AppState;

#[derive(Deserialize)]
pub struct InboxQuery {
    pub status: Option<String>,
}

/// GET /api/inbox — return notifications from SQLite, bootstrapping from GitHub on first call.
pub async fn get_inbox(
    State(state): State<AppState>,
    Query(query): Query<InboxQuery>,
) -> Result<Json<Vec<NotificationRow>>, AppError> {
    // Bootstrap: fetch once inline on first request.
    // AtomicBool prevents concurrent bootstrap within the same AppState.
    // last_fetched_at (set after successful sync) prevents re-bootstrap across AppState instances.
    if state
        .bootstrap_done
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        let has_fetched = queries::get_last_fetched_epoch(&state.pool, "notifications")
            .await?
            .is_some();
        if !has_fetched {
            sync_notifications(&state).await?;
        }
    }

    let results = match query.status.as_deref() {
        Some("archived") => queries::query_archived(&state.pool).await?,
        _ => queries::query_inbox(&state.pool).await?,
    };
    Ok(Json(results))
}
