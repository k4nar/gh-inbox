use std::sync::atomic::Ordering;

use axum::extract::State;
use axum::http::StatusCode;

use crate::api::AppError;
use crate::db::queries;
use crate::github::sync::sync_tick;
use crate::server::AppState;

/// POST /api/sync — trigger an immediate full sync.
///
/// Clears `last_fetched_at` for notifications (forcing a full sync) and runs
/// one sync tick in a fire-and-forget task. Returns 202 immediately. If a
/// background tick is mid-flight, the manual sync waits for it (syncs are
/// serialized by `sync_lock`) rather than being dropped; repeat requests
/// coalesce into the sync already queued.
pub async fn post_sync(State(state): State<AppState>) -> Result<StatusCode, AppError> {
    if state.manual_sync_queued.swap(true, Ordering::SeqCst) {
        return Ok(StatusCode::ACCEPTED);
    }

    tokio::spawn(async move {
        let _lock = state.sync_lock.clone().lock_owned().await;
        // From here on this run serves every request coalesced so far; a new
        // request may queue its own run again.
        state.manual_sync_queued.store(false, Ordering::SeqCst);

        // Force a full sync by clearing the cursor — under the lock, so the
        // cursor stamped by a just-finished tick cannot erase it.
        let _ = queries::clear_last_fetched(&state.pool, "notifications").await;

        sync_tick(&state, &state.tx).await;
    });

    Ok(StatusCode::ACCEPTED)
}
