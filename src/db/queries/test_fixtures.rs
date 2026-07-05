//! Shared row factories for query tests. Callers mutate the returned structs
//! for case-specific values instead of redefining the whole row.

use super::{NotificationRow, PullRequestRow};

pub(crate) fn sample_pull_request(id: i64) -> PullRequestRow {
    PullRequestRow {
        id,
        title: "Fix bug".to_string(),
        repo: "owner/repo".to_string(),
        author: "alice".to_string(),
        author_avatar_url: None,
        url: format!("https://github.com/owner/repo/pull/{id}"),
        ci_status: None,
        last_viewed_at: None,
        body: String::new(),
        state: "open".to_string(),
        head_sha: "abc123".to_string(),
        additions: 0,
        deletions: 0,
        changed_files: 0,
        draft: false,
        merged_at: None,
        teams: None,
        labels: String::from("[]"),
    }
}

pub(crate) fn sample_notification(id: &str) -> NotificationRow {
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
