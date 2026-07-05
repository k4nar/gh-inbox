use sqlx::SqlitePool;

/// A notification row from the database.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct NotificationRow {
    pub id: String,
    pub pr_id: Option<i64>,
    pub title: String,
    pub repository: String,
    pub reason: String,
    pub unread: bool,
    pub archived: bool,
    pub updated_at: String,
}

/// The sync-relevant local state of a tracked notification row.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
struct LocalFlags {
    unread: bool,
    archived: bool,
    locally_unarchived: bool,
    local_write_epoch: Option<i64>,
    updated_at: String,
}

/// What `reconcile` decided to store for one incoming snapshot row.
#[derive(Debug, Clone, PartialEq)]
struct Resolution {
    /// The snapshot brought real news: apply its metadata and the flags below,
    /// and report the row as changed (drives SSE + viewport auto-fetch).
    /// When false, only the sync bookkeeping fields below are written.
    changed: bool,
    unread: bool,
    archived: bool,
    locally_unarchived: bool,
    local_write_epoch: Option<i64>,
}

/// The notification state machine: how one row of a GitHub snapshot combines
/// with local state. Pure so every transition can be unit-tested. Rules:
///
/// - First contact (`local` is None): store the snapshot as the caller shaped
///   it — the cold-start read→archived policy lives in the sync layer.
/// - A local mutation stamped at or after `synced_at` (the sync's *start*
///   time) postdates the snapshot: the snapshot's unread/archived flags are
///   stale and must not overwrite local state. A later sync clears the stamp.
/// - The user's own activity (`your_activity`) never flips read/archived.
/// - New activity (snapshot says unread) revives an archived row.
/// - Metadata only refreshes when the snapshot brought real news; either way
///   GitHub returning the thread lifts unarchive protection (unless the
///   snapshot is stale per the rule above).
fn reconcile(local: Option<&LocalFlags>, incoming: &NotificationRow, synced_at: i64) -> Resolution {
    let Some(local) = local else {
        return Resolution {
            changed: true,
            unread: incoming.unread,
            archived: incoming.archived,
            locally_unarchived: false,
            local_write_epoch: None,
        };
    };

    let stale_snapshot = local.local_write_epoch.is_some_and(|e| e >= synced_at);
    let own_activity = incoming.reason == "your_activity";

    let changed = local.updated_at != incoming.updated_at
        || local.unread != incoming.unread
        || (local.archived && incoming.unread);

    let (unread, archived) = if !changed || stale_snapshot || own_activity {
        (local.unread, local.archived)
    } else {
        (
            incoming.unread,
            // New activity revives an archived thread.
            if incoming.unread {
                false
            } else {
                local.archived
            },
        )
    };

    Resolution {
        changed,
        unread,
        archived,
        locally_unarchived: stale_snapshot && local.locally_unarchived,
        local_write_epoch: if stale_snapshot {
            local.local_write_epoch
        } else {
            None
        },
    }
}

/// Insert or update a notification from a sync snapshot.
/// Returns the number of rows affected (0 if nothing changed, 1 if inserted or
/// updated) — the caller counts non-zero results as inbox changes.
///
/// `synced_at` is always written (even on no-op upserts) so that full-sync
/// reconciliation can archive stale rows by comparing timestamps. All decision
/// logic lives in [`reconcile`]; this function is read-decide-write inside a
/// `BEGIN IMMEDIATE` transaction, so the write lock is held from the read
/// onward and a concurrent local mutation (mark_read/archive) cannot slip in
/// between and be overwritten.
pub async fn upsert_notification(
    pool: &SqlitePool,
    notif: &NotificationRow,
    synced_at: i64,
) -> sqlx::Result<u64> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;

    let local: Option<LocalFlags> = sqlx::query_as(
        "SELECT unread, archived, locally_unarchived, local_write_epoch, updated_at
         FROM notifications WHERE id = ?",
    )
    .bind(&notif.id)
    .fetch_optional(&mut *tx)
    .await?;

    let resolution = reconcile(local.as_ref(), notif, synced_at);

    match (local.is_some(), resolution.changed) {
        // First contact — insert.
        (false, _) => {
            sqlx::query(
                "INSERT INTO notifications (id, pr_id, title, repository, reason, unread, archived, locally_unarchived, local_write_epoch, updated_at, synced_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&notif.id)
            .bind(notif.pr_id)
            .bind(&notif.title)
            .bind(&notif.repository)
            .bind(&notif.reason)
            .bind(resolution.unread)
            .bind(resolution.archived)
            .bind(resolution.locally_unarchived)
            .bind(resolution.local_write_epoch)
            .bind(&notif.updated_at)
            .bind(synced_at)
            .execute(&mut *tx)
            .await?;
        }
        // Real news — refresh metadata and flags.
        (true, true) => {
            sqlx::query(
                "UPDATE notifications SET
                   pr_id = ?, title = ?, repository = ?, reason = ?, updated_at = ?,
                   unread = ?, archived = ?, locally_unarchived = ?, local_write_epoch = ?,
                   synced_at = ?
                 WHERE id = ?",
            )
            .bind(notif.pr_id)
            .bind(&notif.title)
            .bind(&notif.repository)
            .bind(&notif.reason)
            .bind(&notif.updated_at)
            .bind(resolution.unread)
            .bind(resolution.archived)
            .bind(resolution.locally_unarchived)
            .bind(resolution.local_write_epoch)
            .bind(synced_at)
            .bind(&notif.id)
            .execute(&mut *tx)
            .await?;
        }
        // No-op — stamp sync bookkeeping only.
        (true, false) => {
            sqlx::query(
                "UPDATE notifications SET locally_unarchived = ?, local_write_epoch = ?, synced_at = ?
                 WHERE id = ?",
            )
            .bind(resolution.locally_unarchived)
            .bind(resolution.local_write_epoch)
            .bind(synced_at)
            .bind(&notif.id)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    Ok(u64::from(resolution.changed))
}

/// Query all non-archived (inbox) notifications.
pub async fn query_inbox(pool: &SqlitePool) -> sqlx::Result<Vec<NotificationRow>> {
    sqlx::query_as::<_, NotificationRow>(
        "SELECT id, pr_id, title, repository, reason, unread, archived, updated_at
         FROM notifications
         WHERE archived = 0
         ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await
}

/// Query all archived notifications.
pub async fn query_archived(pool: &SqlitePool) -> sqlx::Result<Vec<NotificationRow>> {
    sqlx::query_as::<_, NotificationRow>(
        "SELECT id, pr_id, title, repository, reason, unread, archived, updated_at
         FROM notifications
         WHERE archived = 1
         ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await
}

/// Archive a notification by ID, stamping the local write time so an in-flight
/// sync snapshot cannot revert it. Returns the number of rows affected.
pub async fn archive_notification(pool: &SqlitePool, id: &str, now: i64) -> sqlx::Result<u64> {
    let result = sqlx::query(
        "UPDATE notifications SET archived = 1, locally_unarchived = 0, local_write_epoch = ? WHERE id = ?",
    )
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Unarchive a notification by ID (move back to inbox). Returns the number of rows affected.
/// The row is flagged so full-sync reconciliation does not re-archive it: the thread
/// is marked done on GitHub, so GitHub no longer returns it and it would otherwise
/// look stale on the next full sync.
pub async fn unarchive_notification(pool: &SqlitePool, id: &str, now: i64) -> sqlx::Result<u64> {
    let result = sqlx::query(
        "UPDATE notifications SET archived = 0, locally_unarchived = 1, local_write_epoch = ? WHERE id = ?",
    )
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Mark a notification as read, stamping the local write time so an in-flight
/// sync snapshot cannot flip it back to unread. Returns the number of rows affected.
pub async fn mark_read(pool: &SqlitePool, id: &str, now: i64) -> sqlx::Result<u64> {
    let result =
        sqlx::query("UPDATE notifications SET unread = 0, local_write_epoch = ? WHERE id = ?")
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}

/// Archive all non-archived notifications that were not touched during the
/// current sync cycle (i.e. `synced_at < sync_started_at`).
/// Locally-unarchived rows are exempt — GitHub never returns them again
/// (their thread is marked done), so staleness says nothing about them.
/// Returns the number of rows affected.
pub async fn archive_stale(pool: &SqlitePool, sync_started_at: i64) -> sqlx::Result<u64> {
    let result = sqlx::query(
        "UPDATE notifications SET archived = 1 \
         WHERE archived = 0 AND synced_at < ? AND locally_unarchived = 0",
    )
    .bind(sync_started_at)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod reconcile_tests {
    use super::*;

    fn incoming(unread: bool) -> NotificationRow {
        NotificationRow {
            id: "n1".to_string(),
            pr_id: Some(42),
            title: "T".to_string(),
            repository: "owner/repo".to_string(),
            reason: "review_requested".to_string(),
            unread,
            archived: false,
            updated_at: "2025-01-02T00:00:00Z".to_string(),
        }
    }

    fn local(unread: bool, archived: bool) -> LocalFlags {
        LocalFlags {
            unread,
            archived,
            locally_unarchived: false,
            local_write_epoch: None,
            updated_at: "2025-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn first_contact_stores_snapshot_as_shaped_by_caller() {
        let mut row = incoming(false);
        row.archived = true; // cold-start policy decided upstream
        let r = reconcile(None, &row, 10);
        assert!(r.changed);
        assert!(!r.unread);
        assert!(r.archived);
        assert!(!r.locally_unarchived);
        assert_eq!(r.local_write_epoch, None);
    }

    #[test]
    fn snapshot_applies_when_no_local_write_pending() {
        let r = reconcile(Some(&local(false, false)), &incoming(true), 10);
        assert!(r.changed);
        assert!(r.unread);
        assert!(!r.archived);
    }

    #[test]
    fn stale_snapshot_keeps_local_read_and_archive() {
        // Local write at t=12 postdates a sync that started at t=11.
        let mut l = local(false, true);
        l.local_write_epoch = Some(12);
        let r = reconcile(Some(&l), &incoming(true), 11);
        assert!(!r.unread, "stale snapshot must not revert the read");
        assert!(r.archived, "stale snapshot must not resurrect the archive");
        assert_eq!(r.local_write_epoch, Some(12), "stamp survives stale syncs");
    }

    #[test]
    fn newer_sync_applies_and_clears_stamp() {
        let mut l = local(false, false);
        l.local_write_epoch = Some(12);
        let r = reconcile(Some(&l), &incoming(true), 15);
        assert!(r.unread, "a sync started after the local write is truth");
        assert_eq!(r.local_write_epoch, None);
    }

    #[test]
    fn own_activity_never_flips_flags() {
        let mut row = incoming(true);
        row.reason = "your_activity".to_string();
        let r = reconcile(Some(&local(false, true)), &row, 10);
        assert!(r.changed, "metadata still refreshes");
        assert!(!r.unread);
        assert!(r.archived);
    }

    #[test]
    fn new_activity_revives_archived_row() {
        // Even with identical updated_at/unread, archived + unread = revival.
        let mut l = local(true, true);
        l.updated_at = incoming(true).updated_at;
        let r = reconcile(Some(&l), &incoming(true), 10);
        assert!(r.changed);
        assert!(!r.archived);
    }

    #[test]
    fn read_snapshot_keeps_archived_row_archived() {
        let r = reconcile(Some(&local(false, true)), &incoming(false), 10);
        assert!(r.changed, "updated_at differs");
        assert!(r.archived, "read is not done — no resurrection");
    }

    #[test]
    fn unchanged_snapshot_only_stamps_bookkeeping() {
        let mut l = local(true, false);
        l.updated_at = incoming(true).updated_at;
        let r = reconcile(Some(&l), &incoming(true), 10);
        assert!(!r.changed);
    }

    #[test]
    fn github_returning_thread_lifts_unarchive_protection() {
        let mut l = local(true, false);
        l.updated_at = incoming(true).updated_at;
        l.locally_unarchived = true;
        let r = reconcile(Some(&l), &incoming(true), 10);
        assert!(!r.locally_unarchived);
    }

    #[test]
    fn stale_snapshot_keeps_unarchive_protection() {
        let mut l = local(true, false);
        l.updated_at = incoming(true).updated_at;
        l.locally_unarchived = true;
        l.local_write_epoch = Some(12);
        let r = reconcile(Some(&l), &incoming(true), 11);
        assert!(r.locally_unarchived);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        crate::db::init_with_path(":memory:").await
    }

    fn sample(id: &str) -> NotificationRow {
        NotificationRow {
            id: id.to_string(),
            pr_id: Some(42),
            title: "Fix bug in parser".to_string(),
            repository: "owner/repo".to_string(),
            reason: "review_requested".to_string(),
            unread: true,
            archived: false,
            updated_at: "2025-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn insert_and_query_inbox() {
        let pool = test_pool().await;
        upsert_notification(&pool, &sample("n1"), 1).await.unwrap();
        let inbox = query_inbox(&pool).await.unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].id, "n1");
        assert_eq!(inbox[0].reason, "review_requested");
        assert!(inbox[0].unread);
    }

    #[tokio::test]
    async fn archive_and_unarchive() {
        let pool = test_pool().await;
        upsert_notification(&pool, &sample("n2"), 1).await.unwrap();

        archive_notification(&pool, "n2", 1).await.unwrap();
        assert_eq!(query_inbox(&pool).await.unwrap().len(), 0);
        assert_eq!(query_archived(&pool).await.unwrap().len(), 1);

        unarchive_notification(&pool, "n2", 1).await.unwrap();
        assert_eq!(query_inbox(&pool).await.unwrap().len(), 1);
        assert_eq!(query_archived(&pool).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn upsert_is_idempotent() {
        let pool = test_pool().await;
        let mut notif = sample("n3");
        upsert_notification(&pool, &notif, 1).await.unwrap();
        notif.reason = "mention".to_string();
        notif.updated_at = "2025-01-02T00:00:00Z".to_string();
        upsert_notification(&pool, &notif, 1).await.unwrap();
        let inbox = query_inbox(&pool).await.unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].reason, "mention");
    }

    #[tokio::test]
    async fn upsert_unread_moves_archived_back_to_inbox() {
        let pool = test_pool().await;
        let notif = sample("n4");
        upsert_notification(&pool, &notif, 1).await.unwrap();
        archive_notification(&pool, "n4", 1).await.unwrap();
        // Re-upserting with unread=true should unarchive (new activity).
        // The sync started after the local archive, so the guard does not apply.
        upsert_notification(&pool, &notif, 2).await.unwrap();
        assert_eq!(query_inbox(&pool).await.unwrap().len(), 1);
        assert_eq!(query_archived(&pool).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn upsert_read_keeps_archived_status() {
        let pool = test_pool().await;
        let mut notif = sample("n6");
        upsert_notification(&pool, &notif, 1).await.unwrap();
        archive_notification(&pool, "n6", 1).await.unwrap();
        // Re-upserting with unread=false should preserve archived status
        notif.unread = false;
        upsert_notification(&pool, &notif, 2).await.unwrap();
        assert_eq!(query_inbox(&pool).await.unwrap().len(), 0);
        assert_eq!(query_archived(&pool).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn mark_read_works() {
        let pool = test_pool().await;
        upsert_notification(&pool, &sample("n5"), 1).await.unwrap();
        mark_read(&pool, "n5", 1).await.unwrap();
        let inbox = query_inbox(&pool).await.unwrap();
        assert!(!inbox[0].unread);
    }

    #[tokio::test]
    async fn archive_stale_archives_old_synced_at() {
        let pool = test_pool().await;
        // n1 and n2 synced at t=10, n3 synced at t=5 (stale)
        upsert_notification(&pool, &sample("n1"), 10).await.unwrap();
        upsert_notification(&pool, &sample("n2"), 10).await.unwrap();
        upsert_notification(&pool, &sample("n3"), 5).await.unwrap();

        let count = archive_stale(&pool, 10).await.unwrap();
        assert_eq!(count, 1);

        let archived = query_archived(&pool).await.unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].id, "n3");
    }

    #[tokio::test]
    async fn archive_stale_leaves_current_unchanged() {
        let pool = test_pool().await;
        upsert_notification(&pool, &sample("n1"), 10).await.unwrap();
        upsert_notification(&pool, &sample("n2"), 10).await.unwrap();

        let count = archive_stale(&pool, 10).await.unwrap();
        assert_eq!(count, 0);

        assert_eq!(query_inbox(&pool).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn archive_stale_spares_locally_unarchived() {
        let pool = test_pool().await;
        upsert_notification(&pool, &sample("n1"), 5).await.unwrap();
        archive_notification(&pool, "n1", 6).await.unwrap();
        unarchive_notification(&pool, "n1", 6).await.unwrap();

        // n1 is stale (GitHub no longer returns done threads) but was
        // unarchived locally — reconciliation must leave it in the inbox.
        let count = archive_stale(&pool, 10).await.unwrap();
        assert_eq!(count, 0);
        assert_eq!(query_inbox(&pool).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn new_activity_clears_unarchive_protection() {
        let pool = test_pool().await;
        let mut notif = sample("n1");
        upsert_notification(&pool, &notif, 5).await.unwrap();
        archive_notification(&pool, "n1", 6).await.unwrap();
        unarchive_notification(&pool, "n1", 6).await.unwrap();

        // GitHub returns the thread again (new activity) — protection lifts,
        // so a later full sync that no longer sees the thread re-archives it.
        notif.updated_at = "2025-01-02T00:00:00Z".to_string();
        upsert_notification(&pool, &notif, 8).await.unwrap();
        let count = archive_stale(&pool, 10).await.unwrap();
        assert_eq!(count, 1);
        assert_eq!(query_archived(&pool).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn stale_sync_does_not_revert_fresh_local_read() {
        let pool = test_pool().await;
        // Sync starts at t=10 and snapshots the notification as unread.
        upsert_notification(&pool, &sample("n1"), 10).await.unwrap();
        // User marks it read at t=12, while the next sync (started t=11,
        // snapshot predates the click) is still in flight.
        mark_read(&pool, "n1", 12).await.unwrap();
        upsert_notification(&pool, &sample("n1"), 11).await.unwrap();

        let inbox = query_inbox(&pool).await.unwrap();
        assert!(!inbox[0].unread, "stale snapshot must not revert the read");
    }

    #[tokio::test]
    async fn stale_sync_does_not_resurrect_fresh_local_archive() {
        let pool = test_pool().await;
        upsert_notification(&pool, &sample("n1"), 10).await.unwrap();
        // User archives at t=12; in-flight sync started at t=11 still has the
        // thread as unread in its snapshot.
        archive_notification(&pool, "n1", 12).await.unwrap();
        upsert_notification(&pool, &sample("n1"), 11).await.unwrap();

        assert_eq!(query_inbox(&pool).await.unwrap().len(), 0);
        assert_eq!(query_archived(&pool).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn newer_sync_overrides_local_state_and_clears_stamp() {
        let pool = test_pool().await;
        let mut notif = sample("n1");
        upsert_notification(&pool, &notif, 10).await.unwrap();
        mark_read(&pool, "n1", 12).await.unwrap();

        // A sync that started after the local write reflects real GitHub
        // state (new activity arrived): it may flip the flags again.
        notif.updated_at = "2025-01-02T00:00:00Z".to_string();
        upsert_notification(&pool, &notif, 15).await.unwrap();
        let inbox = query_inbox(&pool).await.unwrap();
        assert!(inbox[0].unread, "newer sync state must apply");

        // The stamp was cleared, so an even later stale-looking snapshot
        // isn't blocked by the old local write.
        mark_read(&pool, "n1", 20).await.unwrap();
        upsert_notification(&pool, &notif, 21).await.unwrap();
        let inbox = query_inbox(&pool).await.unwrap();
        assert!(inbox[0].unread);
    }

    #[tokio::test]
    async fn archive_stale_skips_already_archived() {
        let pool = test_pool().await;
        upsert_notification(&pool, &sample("n1"), 5).await.unwrap();
        archive_notification(&pool, "n1", 6).await.unwrap();

        // n1 is stale but already archived — rows_affected should be 0
        let count = archive_stale(&pool, 10).await.unwrap();
        assert_eq!(count, 0);
    }
}

#[cfg(test)]
mod own_activity_tests {
    use super::*;

    async fn pool() -> SqlitePool {
        crate::db::init_with_path(":memory:").await
    }

    fn row(reason: &str, unread: bool, archived: bool) -> NotificationRow {
        NotificationRow {
            id: "n1".to_string(),
            pr_id: Some(1),
            title: "PR title".to_string(),
            repository: "owner/repo".to_string(),
            reason: reason.to_string(),
            unread,
            archived,
            updated_at: "2025-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn own_activity_insert_starts_as_read() {
        let pool = pool().await;
        upsert_notification(&pool, &row("your_activity", false, false), 1)
            .await
            .unwrap();
        let rows = query_inbox(&pool).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].unread);
    }

    #[tokio::test]
    async fn own_activity_does_not_make_read_notification_unread() {
        let pool = pool().await;
        upsert_notification(&pool, &row("review_requested", false, false), 1)
            .await
            .unwrap();
        let mut r = row("your_activity", false, false);
        r.updated_at = "2025-01-02T00:00:00Z".to_string();
        upsert_notification(&pool, &r, 1).await.unwrap();
        let rows = query_inbox(&pool).await.unwrap();
        assert!(!rows[0].unread);
    }

    #[tokio::test]
    async fn own_activity_preserves_unread_when_already_unread() {
        let pool = pool().await;
        upsert_notification(&pool, &row("review_requested", true, false), 1)
            .await
            .unwrap();
        let mut r = row("your_activity", false, false);
        r.updated_at = "2025-01-02T00:00:00Z".to_string();
        upsert_notification(&pool, &r, 1).await.unwrap();
        let rows = query_inbox(&pool).await.unwrap();
        assert!(rows[0].unread);
    }

    #[tokio::test]
    async fn own_activity_preserves_archived_state() {
        let pool = pool().await;
        upsert_notification(&pool, &row("review_requested", false, true), 1)
            .await
            .unwrap();
        let mut r = row("your_activity", false, false);
        r.updated_at = "2025-01-02T00:00:00Z".to_string();
        upsert_notification(&pool, &r, 1).await.unwrap();
        let archived = query_archived(&pool).await.unwrap();
        assert_eq!(archived.len(), 1, "notification should remain archived");
    }
}
