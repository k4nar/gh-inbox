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
            SyncError::GitHub(e) => write!(f, "GitHub error: {}", super::error_chain(e)),
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
    /// The cadence GitHub advertised via `x-poll-interval`, when it sent one.
    pub poll_interval: Option<Duration>,
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

    let feed = if is_full_sync {
        // Unconditional by design: a 304 would skip reconciliation, which
        // works by absence. See fetch_all_notifications.
        super::fetch_all_notifications(&state.github).await?
    } else {
        // GitHub filters `since` by *its* clock while the cursor comes from
        // ours; overlap the window by a margin so moderate clock skew cannot
        // silently drop notifications. Re-fetched ones are no-op upserts.
        let since_iso = epoch_to_iso(last_fetched.unwrap() - SINCE_OVERLAP_SECS)?;
        let validator = state.notifications_last_modified.read().await.clone();
        super::fetch_notifications_since(&state.github, &since_iso, validator.as_deref()).await?
    };

    if feed.not_modified {
        // Nothing changed since the last poll. Still advance the cursor: this
        // snapshot is authoritative for "no changes", so the next incremental
        // window can start from here.
        tracing::debug!("notification feed unchanged (304)");
        queries::set_last_fetched_epoch(&state.pool, "notifications", now).await?;
        return Ok(SyncResult {
            changed: Vec::new(),
            reconciled: 0,
            poll_interval: feed.poll_interval,
        });
    }

    let notifications = feed.notifications;

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
        // sync tick still lands in the inbox ("mark read but keep the PR in
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

    // Only now, with the snapshot applied and the cursor stamped, is the
    // validator safe to keep. The whole conditional-poll scheme rests on one
    // invariant: everything up to the stored validator has been applied
    // locally. Storing it before these writes breaks it — a failure in
    // between would leave the validator ahead of the data, and the next
    // poll's 304 would stamp the cursor past notifications never written,
    // losing them for good. A 304 carries no validator, so the stored one
    // survives that path untouched, which is what we want.
    if feed.last_modified.is_some() {
        *state.notifications_last_modified.write().await = feed.last_modified.clone();
    }

    Ok(SyncResult {
        changed,
        reconciled,
        poll_interval: feed.poll_interval,
    })
}

/// One tick of the sync loop: broadcast Started, sync, broadcast the outcome.
/// Lock handling lives with the callers.
///
/// Returns the cadence GitHub advertised via `x-poll-interval`, so the loop
/// can pace itself by it. `None` when the sync failed or the header was absent.
pub(crate) async fn sync_tick(
    state: &AppState,
    tx: &broadcast::Sender<SyncEvent>,
) -> Option<Duration> {
    // Ignore send errors — they just mean no clients are listening
    let _ = tx.send(SyncEvent::SyncStatus {
        status: SyncStatusKind::Started,
    });

    match sync_notifications(state).await {
        Ok(SyncResult {
            changed,
            reconciled,
            poll_interval,
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
            poll_interval
        }
        Err(e) => {
            tracing::warn!(error = %e, "notification sync failed");
            let _ = tx.send(SyncEvent::SyncStatus {
                status: SyncStatusKind::Errored {
                    message: format!("{e}"),
                },
            });
            None
        }
    }
}

/// Run the background notification sync loop.
/// Fetches notifications immediately, then every `interval`.
/// Sends events to `tx` for SSE clients.
/// When notifications change for PRs in the viewport, auto-fetches their data.
///
/// `interval` is a floor, not the cadence: GitHub advertises its own minimum
/// via `x-poll-interval` and raises it when loaded, so each tick waits for
/// whichever of the two is longer.
pub async fn run_sync_loop(state: AppState, tx: broadcast::Sender<SyncEvent>, interval: Duration) {
    // Always start with a full sync so that any state accumulated while the
    // service was stopped (notifications cleared on GitHub, etc.) is reconciled
    // immediately on startup rather than waiting up to 2h.
    let _ = queries::clear_last_fetched(&state.pool, "notifications").await;

    // Last cadence GitHub asked for. Kept across ticks so a skipped tick or a
    // failed sync does not silently drop back to polling at the floor.
    let mut advertised: Option<Duration> = None;

    loop {
        // Take the same lock POST /api/sync uses, so a manual sync can never
        // start while a loop tick is mid-flight (and vice versa): two
        // interleaved syncs run reconciliation against each other's snapshots
        // and can spuriously archive rows. Skip the tick if the lock is taken
        // (the Mutex is fair, so try_lock also fails while a manual sync is
        // queued — the tick yields to it).
        if let Ok(guard) = state.sync_lock.clone().try_lock_owned() {
            // Run the tick in its own task: a panic surfaces here as a
            // JoinError instead of unwinding through — and killing — the loop.
            // The guard travels into the task so the lock is released either way.
            let tick = tokio::spawn({
                let state = state.clone();
                let tx = tx.clone();
                async move {
                    let _guard = guard;
                    sync_tick(&state, &tx).await
                }
            });
            match tick.await {
                Ok(Some(from_github)) => {
                    if advertised != Some(from_github) {
                        tracing::debug!(
                            seconds = from_github.as_secs(),
                            "GitHub advertised a poll interval"
                        );
                    }
                    advertised = Some(from_github);
                }
                Ok(None) => {}
                Err(e) => tracing::error!(error = %e, "sync tick panicked"),
            }
        }

        tokio::time::sleep(next_delay(interval, advertised)).await;
    }
}

/// How long to wait before the next tick: never faster than GitHub allows,
/// never faster than the configured cadence either.
fn next_delay(configured: Duration, advertised: Option<Duration>) -> Duration {
    match advertised {
        Some(from_github) => configured.max(from_github),
        None => configured,
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
