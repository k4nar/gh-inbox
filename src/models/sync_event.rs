use serde::{Deserialize, Serialize};

/// The four possible PR statuses, matching the SQL CASE expression in INBOX_ENRICHED_SQL.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrStatus {
    Open,
    Draft,
    Merged,
    Closed,
}

/// Events broadcast from the sync loop to SSE clients.
/// The SSE `event:` line carries the type discriminator;
/// the JSON `data:` carries only the payload.
#[derive(Debug, Clone)]
pub enum SyncEvent {
    NewNotifications { count: usize },
    SyncStatus { status: SyncStatusKind },
    PrTeamsUpdated(PrTeamsUpdatedData),
    PrInfoUpdated(PrInfoUpdatedData),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncStatusKind {
    Started,
    Completed,
    Errored { message: String },
}

/// Payload serialized into the SSE `data:` field for NewNotifications.
#[derive(Serialize)]
pub struct NewNotificationsData {
    pub count: usize,
}

/// Payload serialized into the SSE `data:` field for SyncStatus.
#[derive(Serialize)]
pub struct SyncStatusData {
    pub status: SyncStatusKind,
}

/// Payload serialized into the SSE `data:` field for PrTeamsUpdated.
#[derive(Debug, Clone, Serialize)]
pub struct PrTeamsUpdatedData {
    pub pr_id: i64,
    pub teams: Vec<String>,
}

/// Payload serialized into the SSE `data:` field for PrInfoUpdated.
#[derive(Debug, Clone, Serialize)]
pub struct PrInfoUpdatedData {
    pub pr_id: i64,
    pub repository: String,
    pub author: String,
    pub pr_status: PrStatus,
    /// None means last_viewed_at is NULL (first visit); Some(n) = n new commits since last visit.
    pub new_commits: Option<i64>,
    /// None means last_viewed_at is NULL; Some([]) = no new comments.
    pub new_comments: Option<Vec<PrNewComment>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrNewComment {
    pub author: String,
    pub count: i64,
}
