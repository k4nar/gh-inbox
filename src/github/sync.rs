use std::time::Duration;

use tokio::sync::broadcast;

use crate::api::inbox::sync_notifications;
use crate::models::{SyncEvent, SyncStatusKind};
use crate::server::AppState;

/// Run the background notification sync loop.
/// Fetches notifications immediately, then every `interval` seconds.
/// Sends events to `tx` for SSE clients.
pub async fn run_sync_loop(state: AppState, tx: broadcast::Sender<SyncEvent>) {
    let interval_secs: u64 = std::env::var("GH_INBOX_SYNC_INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    let interval = Duration::from_secs(interval_secs);

    loop {
        // Ignore send errors — they just mean no clients are listening
        let _ = tx.send(SyncEvent::SyncStatus {
            status: SyncStatusKind::Started,
        });

        match sync_notifications(&state).await {
            Ok(count) => {
                if count > 0 {
                    let _ = tx.send(SyncEvent::NewNotifications { count });
                }
                let _ = tx.send(SyncEvent::SyncStatus {
                    status: SyncStatusKind::Completed,
                });
            }
            Err(e) => {
                eprintln!("Sync error: {e:?}");
                let _ = tx.send(SyncEvent::SyncStatus {
                    status: SyncStatusKind::Errored {
                        message: format!("{e:?}"),
                    },
                });
            }
        }

        tokio::time::sleep(interval).await;
    }
}
