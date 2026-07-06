use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::sync::broadcast;

use crate::clock::now_epoch;
use crate::db::queries;
use crate::models::{SyncEvent, SyncStatusKind};
use crate::server::AppState;

/// Error type for sync operations — avoids a dependency on `api::AppError`.
#[derive(Debug)]
pub enum SyncError {
    GitHub(reqwest::Error),
    Database(sqlx::Error),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::GitHub(e) => write!(f, "GitHub error: {e}"),
            SyncError::Database(e) => write!(f, "Database error: {e}"),
        }
    }
}

impl From<reqwest::Error> for SyncError {
    fn from(err: reqwest::Error) -> Self {
        SyncError::GitHub(err)
    }
}

impl From<sqlx::Error> for SyncError {
    fn from(err: sqlx::Error) -> Self {
        SyncError::Database(err)
    }
}

const FULL_SYNC_THRESHOLD_SECS: i64 = 2 * 60 * 60; // 2 hours

/// Clock-skew tolerance subtracted from the incremental `since` cursor.
const SINCE_OVERLAP_SECS: i64 = 60;

fn epoch_to_iso(epoch: i64) -> Result<String, SyncError> {
    chrono::DateTime::from_timestamp(epoch, 0)
        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .ok_or_else(|| {
            SyncError::Database(sqlx::Error::Protocol(format!("epoch {epoch} out of range")))
        })
}

/// A notification that changed during sync.
pub struct ChangedNotification {
    pub repository: String,
    pub pr_id: Option<i64>,
}

/// Result of a sync operation.
pub struct SyncResult {
    /// Notifications that were upserted (new or updated).
    pub changed: Vec<ChangedNotification>,
    /// Number of notifications archived during full-sync reconciliation.
    pub reconciled: usize,
}

/// Fetch notifications from GitHub and upsert into the database.
///
/// **Full sync** (first run or last fetch >2h ago): fetches all pages and
/// archives any local notification that GitHub no longer returns.
///
/// **Incremental sync** (recent last fetch): fetches only notifications
/// changed since the last fetch using the `since=` parameter.
pub async fn sync_notifications(state: &AppState) -> Result<SyncResult, SyncError> {
    let last_fetched = queries::get_last_fetched_epoch(&state.pool, "notifications").await?;
    let now = now_epoch();

    let is_full_sync = last_fetched
        .map(|t| now - t > FULL_SYNC_THRESHOLD_SECS)
        .unwrap_or(true);

    let notifications = if is_full_sync {
        super::fetch_all_notifications(&state.github).await?
    } else {
        // GitHub filters `since` by *its* clock while the cursor comes from
        // ours; overlap the window by a margin so moderate clock skew cannot
        // silently drop notifications. Re-fetched ones are no-op upserts.
        let since_iso = epoch_to_iso(last_fetched.unwrap() - SINCE_OVERLAP_SECS)?;
        super::fetch_notifications_since(&state.github, &since_iso).await?
    };

    let mut changed = Vec::new();

    for notif in &notifications {
        // Only PullRequest subjects carry a PR number; an Issue URL like
        // …/issues/431 would otherwise be mistaken for PR #431 of the repo.
        let pr_id = (notif.subject.subject_type == "PullRequest")
            .then_some(notif.subject.url.as_deref())
            .flatten()
            .and_then(|url| url.rsplit('/').next())
            .and_then(|s| s.parse::<i64>().ok());

        let row = queries::NotificationRow {
            id: notif.id.clone(),
            pr_id,
            title: notif.subject.title.clone(),
            repository: notif.repository.full_name.clone(),
            reason: notif.reason.clone(),
            unread: notif.unread && notif.reason != "your_activity",
            // Ignored for sync rows: reconcile() owns the archived decision
            // (see archive_read_on_first_contact below).
            archived: false,
            updated_at: notif.updated_at.clone(),
        };

        // Cold-start policy, applied only during FULL syncs: first-contact
        // threads GitHub already reports read were handled outside gh-inbox
        // and start archived instead of flooding the inbox. Incremental ticks
        // are exempt so a brand-new notification read on the web within one
        // 30s tick still lands in the inbox ("mark read but keep the PR in
        // the list"). New activity revives an archived row via reconcile().
        let rows_affected =
            queries::upsert_notification(&state.pool, &row, now, is_full_sync).await?;
        if rows_affected > 0 {
            changed.push(ChangedNotification {
                repository: notif.repository.full_name.clone(),
                pr_id,
            });
        }
    }

    // Full sync reconciliation: archive notifications GitHub no longer returns.
    // Guard: skip if no notifications were returned to avoid archiving everything
    // on an unexpected empty response.
    let reconciled = if is_full_sync && !notifications.is_empty() {
        queries::archive_stale(&state.pool, now).await? as usize
    } else {
        0
    };

    // Record the sync *start* time, not completion: notifications updated while
    // the multi-page fetch was in flight are not in this snapshot and must fall
    // inside the next incremental window.
    queries::set_last_fetched_epoch(&state.pool, "notifications", now).await?;

    Ok(SyncResult {
        changed,
        reconciled,
    })
}

/// Clears `sync_in_progress` on drop, so the flag is released even when the
/// sync task panics. A stuck flag would silently disable both the background
/// loop and POST /api/sync for the rest of the session.
pub struct SyncInProgressGuard(pub std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Drop for SyncInProgressGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// One tick of the sync loop: broadcast Started, sync, broadcast the outcome.
/// Flag handling lives with the callers.
async fn sync_tick(state: &AppState, tx: &broadcast::Sender<SyncEvent>) {
    // Ignore send errors — they just mean no clients are listening
    let _ = tx.send(SyncEvent::SyncStatus {
        status: SyncStatusKind::Started,
    });

    match sync_notifications(state).await {
        Ok(SyncResult {
            changed,
            reconciled,
        }) => {
            let count = changed.len() + reconciled;
            if count > 0 {
                tracing::info!(count, "inbox changed");
                let _ = tx.send(SyncEvent::NewNotifications { count });

                // Auto-fetch PR data for changed notifications in the viewport.
                auto_fetch_viewport_prs(state, tx, &changed).await;
            }
            let _ = tx.send(SyncEvent::SyncStatus {
                status: SyncStatusKind::Completed,
            });
        }
        Err(e) => {
            tracing::warn!(error = %e, "notification sync failed");
            let _ = tx.send(SyncEvent::SyncStatus {
                status: SyncStatusKind::Errored {
                    message: format!("{e}"),
                },
            });
        }
    }
}

/// Run the background notification sync loop.
/// Fetches notifications immediately, then every `interval`.
/// Sends events to `tx` for SSE clients.
/// When notifications change for PRs in the viewport, auto-fetches their data.
pub async fn run_sync_loop(state: AppState, tx: broadcast::Sender<SyncEvent>, interval: Duration) {
    // Always start with a full sync so that any state accumulated while the
    // service was stopped (notifications cleared on GitHub, etc.) is reconciled
    // immediately on startup rather than waiting up to 2h.
    let _ = queries::clear_last_fetched(&state.pool, "notifications").await;

    loop {
        // Take the same guard POST /api/sync uses, so a manual sync can never
        // start while a loop tick is mid-flight (and vice versa): two
        // interleaved syncs run reconciliation against each other's snapshots
        // and can spuriously archive rows. Skip the tick if the flag is held.
        if state
            .sync_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            // Run the tick in its own task: a panic surfaces here as a
            // JoinError instead of unwinding through — and killing — the loop.
            // The guard travels into the task so the flag is released either way.
            let guard = SyncInProgressGuard(state.sync_in_progress.clone());
            let tick = tokio::spawn({
                let state = state.clone();
                let tx = tx.clone();
                async move {
                    let _guard = guard;
                    sync_tick(&state, &tx).await;
                }
            });
            if let Err(e) = tick.await {
                tracing::error!(error = %e, "sync tick panicked");
            }
        }

        tokio::time::sleep(interval).await;
    }
}

/// For changed notifications whose PR is currently in the viewport,
/// fetch fresh PR data and broadcast SSE updates.
pub(crate) async fn auto_fetch_viewport_prs(
    state: &AppState,
    tx: &broadcast::Sender<SyncEvent>,
    changed: &[ChangedNotification],
) {
    let viewport = state.viewport_prs.read().await;
    if viewport.is_empty() {
        return;
    }

    for notif in changed {
        let pr_id = match notif.pr_id {
            Some(id) => id,
            None => continue,
        };
        if !viewport.contains(&(notif.repository.clone(), pr_id)) {
            continue;
        }

        if let Err(e) = crate::github::pr_cache::refresh_pr_and_broadcast(
            &state.pool,
            &state.github,
            tx,
            &notif.repository,
            pr_id,
        )
        .await
        {
            tracing::warn!(
                repository = %notif.repository,
                pr_id,
                error = %e,
                "auto-fetch viewport PR failed"
            );
        }
    }
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
