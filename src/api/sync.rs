use std::sync::atomic::Ordering;

use axum::extract::State;
use axum::http::StatusCode;

use crate::api::AppError;
use crate::db::queries;
use crate::github::sync::{SyncResult, auto_fetch_viewport_prs, sync_notifications};
use crate::models::{SyncEvent, SyncStatusKind};
use crate::server::AppState;

/// POST /api/sync — trigger an immediate full sync.
///
/// Clears `last_fetched_at` for notifications (forcing a full sync) and spawns
/// `sync_notifications` in a fire-and-forget task. Returns 202 immediately.
/// If a sync is already in progress, returns 202 without spawning a second one.
pub async fn post_sync(State(state): State<AppState>) -> Result<StatusCode, AppError> {
    // Guard: only one manual sync at a time.
    if state
        .sync_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(StatusCode::ACCEPTED);
    }

    let state_clone = state.clone();
    tokio::spawn(async move {
        // Force full sync by clearing last_fetched_at.
        let _ = queries::clear_last_fetched(&state_clone.pool, "notifications").await;

        let _ = state_clone.tx.send(SyncEvent::SyncStatus {
            status: SyncStatusKind::Started,
        });

        match sync_notifications(&state_clone).await {
            Ok(SyncResult {
                changed,
                reconciled,
            }) => {
                let count = changed.len() + reconciled as usize;
                if count > 0 {
                    let _ = state_clone.tx.send(SyncEvent::NewNotifications { count });
                    auto_fetch_viewport_prs(&state_clone, &state_clone.tx, &changed).await;
                }
                let _ = state_clone.tx.send(SyncEvent::SyncStatus {
                    status: SyncStatusKind::Completed,
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, "manual sync failed");
                let _ = state_clone.tx.send(SyncEvent::SyncStatus {
                    status: SyncStatusKind::Errored {
                        message: format!("{e:?}"),
                    },
                });
            }
        }

        state_clone.sync_in_progress.store(false, Ordering::SeqCst);
    });

    Ok(StatusCode::ACCEPTED)
}
