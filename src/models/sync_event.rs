use serde::Serialize;

/// Events broadcast from the sync loop to SSE clients.
/// The SSE `event:` line carries the type discriminator;
/// the JSON `data:` carries only the payload.
#[derive(Debug, Clone)]
pub enum SyncEvent {
    NewNotifications { count: usize },
    SyncStatus { status: SyncStatusKind },
    PrTeamsUpdated(PrTeamsUpdatedData),
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
