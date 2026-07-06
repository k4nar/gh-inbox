use super::*;

async fn test_pool() -> SqlitePool {
    crate::db::init_with_path(":memory:").await
}

fn sample(id: i64) -> PullRequestRow {
    let mut pr = crate::db::queries::test_fixtures::sample_pull_request(id);
    pr.ci_status = Some("success".to_string());
    pr.body = "PR body".to_string();
    pr.additions = 10;
    pr.deletions = 3;
    pr.changed_files = 2;
    pr
}

#[tokio::test]
async fn upsert_roundtrip() {
    let pool = test_pool().await;
    upsert_pull_request(&pool, &sample(100)).await.unwrap();
    let row = get_pull_request(&pool, "owner/repo", 100)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.title, "Fix bug");
    assert_eq!(row.ci_status, Some("success".to_string()));
    assert_eq!(row.body, "PR body");
    assert_eq!(row.state, "open");
    assert_eq!(row.head_sha, "abc123");
    assert_eq!(row.additions, 10);
    assert_eq!(row.deletions, 3);
    assert_eq!(row.changed_files, 2);
}

#[tokio::test]
async fn same_number_in_different_repos_does_not_collide() {
    let pool = test_pool().await;
    let pr_a = sample(100); // repo "owner/repo"
    let mut pr_b = sample(100);
    pr_b.repo = "other/repo".to_string();
    pr_b.title = "Other PR".to_string();
    upsert_pull_request(&pool, &pr_a).await.unwrap();
    upsert_pull_request(&pool, &pr_b).await.unwrap();

    let row_a = get_pull_request(&pool, "owner/repo", 100)
        .await
        .unwrap()
        .unwrap();
    let row_b = get_pull_request(&pool, "other/repo", 100)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row_a.title, "Fix bug");
    assert_eq!(row_b.title, "Other PR");

    // Scoped updates must not leak into the same number of another repo.
    update_last_viewed_at(&pool, "owner/repo", 100)
        .await
        .unwrap();
    update_teams(&pool, "owner/repo", 100, "[\"acme/platform\"]")
        .await
        .unwrap();
    let row_b = get_pull_request(&pool, "other/repo", 100)
        .await
        .unwrap()
        .unwrap();
    assert!(row_b.last_viewed_at.is_none());
    assert!(row_b.teams.is_none());
}

#[tokio::test]
async fn get_not_found() {
    let pool = test_pool().await;
    let result = get_pull_request(&pool, "owner/repo", 999).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn upsert_does_not_overwrite_teams() {
    let pool = test_pool().await;
    upsert_pull_request(&pool, &sample(200)).await.unwrap();
    // Manually set teams
    sqlx::query("UPDATE pull_requests SET teams = '[\"acme/platform\"]' WHERE repo = 'owner/repo' AND id = ?")
        .bind(200_i64)
        .execute(&pool)
        .await
        .unwrap();
    // Upsert again
    upsert_pull_request(&pool, &sample(200)).await.unwrap();
    let row = get_pull_request(&pool, "owner/repo", 200)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.teams.as_deref(), Some("[\"acme/platform\"]"));
}

#[tokio::test]
async fn upsert_updates_draft_and_merged_at() {
    let pool = test_pool().await;
    upsert_pull_request(&pool, &sample(300)).await.unwrap(); // draft=false, merged_at=None
    let mut updated = sample(300);
    updated.draft = true;
    updated.merged_at = Some("2025-06-01T00:00:00Z".to_string());
    upsert_pull_request(&pool, &updated).await.unwrap();
    let row = get_pull_request(&pool, "owner/repo", 300)
        .await
        .unwrap()
        .unwrap();
    assert!(row.draft);
    assert_eq!(row.merged_at.as_deref(), Some("2025-06-01T00:00:00Z"));
}

#[tokio::test]
async fn update_last_viewed_at_works() {
    let pool = test_pool().await;
    upsert_pull_request(&pool, &sample(42)).await.unwrap();
    assert!(
        get_pull_request(&pool, "owner/repo", 42)
            .await
            .unwrap()
            .unwrap()
            .last_viewed_at
            .is_none()
    );
    update_last_viewed_at(&pool, "owner/repo", 42)
        .await
        .unwrap();
    assert!(
        get_pull_request(&pool, "owner/repo", 42)
            .await
            .unwrap()
            .unwrap()
            .last_viewed_at
            .is_some()
    );
}

#[tokio::test]
async fn query_inbox_enriched_returns_inbox_items() {
    let pool = test_pool().await;
    let notif = crate::db::queries::NotificationRow {
        id: "n1".to_string(),
        pr_id: Some(42),
        title: "Fix bug".to_string(),
        repository: "owner/repo".to_string(),
        reason: "review_requested".to_string(),
        unread: true,
        archived: false,
        updated_at: "2025-01-01T00:00:00Z".to_string(),
    };
    crate::db::queries::upsert_notification(&pool, &notif, 1, false)
        .await
        .unwrap();
    upsert_pull_request(
        &pool,
        &PullRequestRow {
            id: 42,
            title: "Fix bug".to_string(),
            repo: "owner/repo".to_string(),
            author: "alice".to_string(),
            author_avatar_url: None,
            url: "https://github.com/owner/repo/pull/42".to_string(),
            ci_status: None,
            last_viewed_at: Some("2025-01-01T00:00:00Z".to_string()),
            body: String::new(),
            body_html: String::new(),
            state: "open".to_string(),
            head_sha: "abc".to_string(),
            additions: 0,
            deletions: 0,
            changed_files: 0,
            draft: false,
            merged_at: None,
            teams: None,
            labels: String::from("[]"),
        },
    )
    .await
    .unwrap();

    let (items, _) = query_inbox_enriched_paginated(&pool, 100, 0, &FilterParams::default())
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].author.as_deref(), Some("alice"));
    assert_eq!(items[0].pr_status.as_deref(), Some("open"));
    assert!(items[0].teams.is_none()); // NULL → None
}

#[tokio::test]
async fn pr_status_draft() {
    let pool = test_pool().await;
    let notif = crate::db::queries::NotificationRow {
        id: "n2".to_string(),
        pr_id: Some(43),
        title: "Draft PR".to_string(),
        repository: "owner/repo".to_string(),
        reason: "review_requested".to_string(),
        unread: true,
        archived: false,
        updated_at: "2025-01-02T00:00:00Z".to_string(),
    };
    crate::db::queries::upsert_notification(&pool, &notif, 1, false)
        .await
        .unwrap();
    upsert_pull_request(
        &pool,
        &PullRequestRow {
            id: 43,
            title: "Draft PR".to_string(),
            repo: "owner/repo".to_string(),
            author: "bob".to_string(),
            author_avatar_url: None,
            url: "u".to_string(),
            ci_status: None,
            last_viewed_at: Some("2025-01-01T00:00:00Z".to_string()),
            body: String::new(),
            body_html: String::new(),
            state: "open".to_string(),
            head_sha: "x".to_string(),
            additions: 0,
            deletions: 0,
            changed_files: 0,
            draft: true,
            merged_at: None,
            teams: None,
            labels: String::from("[]"),
        },
    )
    .await
    .unwrap();
    let (items, _) = query_inbox_enriched_paginated(&pool, 100, 0, &FilterParams::default())
        .await
        .unwrap();
    let item = items.iter().find(|i| i.pr_id == Some(43)).unwrap();
    assert_eq!(item.pr_status.as_deref(), Some("draft"));
}

#[tokio::test]
async fn pr_status_merged() {
    let pool = test_pool().await;
    let notif = crate::db::queries::NotificationRow {
        id: "n3".to_string(),
        pr_id: Some(44),
        title: "Merged PR".to_string(),
        repository: "owner/repo".to_string(),
        reason: "mention".to_string(),
        unread: false,
        archived: false,
        updated_at: "2025-01-03T00:00:00Z".to_string(),
    };
    crate::db::queries::upsert_notification(&pool, &notif, 1, false)
        .await
        .unwrap();
    upsert_pull_request(
        &pool,
        &PullRequestRow {
            id: 44,
            title: "Merged".to_string(),
            repo: "owner/repo".to_string(),
            author: "carol".to_string(),
            author_avatar_url: None,
            url: "u".to_string(),
            ci_status: None,
            last_viewed_at: Some("2025-01-01T00:00:00Z".to_string()),
            body: String::new(),
            body_html: String::new(),
            state: "closed".to_string(),
            head_sha: "y".to_string(),
            additions: 0,
            deletions: 0,
            changed_files: 0,
            draft: false,
            merged_at: Some("2025-01-02T12:00:00Z".to_string()),
            teams: None,
            labels: String::from("[]"),
        },
    )
    .await
    .unwrap();
    let (items, _) = query_inbox_enriched_paginated(&pool, 100, 0, &FilterParams::default())
        .await
        .unwrap();
    let item = items.iter().find(|i| i.pr_id == Some(44)).unwrap();
    assert_eq!(item.pr_status.as_deref(), Some("merged"));
}

#[tokio::test]
async fn get_pr_activity_returns_none_when_never_viewed() {
    let pool = test_pool().await;
    upsert_pull_request(&pool, &sample(10)).await.unwrap(); // last_viewed_at = None
    let (commits, comments) = get_pr_activity(&pool, 10, "owner/repo").await.unwrap();
    assert!(commits.is_none(), "expect None when last_viewed_at is NULL");
    assert!(comments.is_none());
}

#[tokio::test]
async fn get_pr_activity_returns_zero_after_viewing() {
    let pool = test_pool().await;
    upsert_pull_request(&pool, &sample(20)).await.unwrap();
    update_last_viewed_at(&pool, "owner/repo", 20)
        .await
        .unwrap();
    let (commits, comments) = get_pr_activity(&pool, 20, "owner/repo").await.unwrap();
    assert_eq!(commits, Some(0));
    // No comments yet, but the field should be Some (not None) now that last_viewed_at is set
    assert!(
        comments.is_some(),
        "expect Some([]) when last_viewed_at is set and no new comments"
    );
}

#[tokio::test]
async fn get_pr_activity_returns_none_for_missing_pr() {
    let pool = test_pool().await;
    let (commits, comments) = get_pr_activity(&pool, 999, "owner/repo").await.unwrap();
    assert!(commits.is_none());
    assert!(comments.is_none());
}

#[tokio::test]
async fn query_inbox_enriched_paginates() {
    let pool = test_pool().await;
    for i in 1..=3 {
        let notif = crate::db::queries::NotificationRow {
            id: format!("n{i}"),
            pr_id: Some(i),
            title: format!("PR {i}"),
            repository: "owner/repo".to_string(),
            reason: "review_requested".to_string(),
            unread: true,
            archived: false,
            updated_at: format!("2025-01-0{i}T00:00:00Z"),
        };
        crate::db::queries::upsert_notification(&pool, &notif, 1, false)
            .await
            .unwrap();
    }
    let (items, total) = query_inbox_enriched_paginated(&pool, 2, 0, &FilterParams::default())
        .await
        .unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(total, 3);
    let (items, total) = query_inbox_enriched_paginated(&pool, 2, 2, &FilterParams::default())
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(total, 3);
    let (items, total) = query_inbox_enriched_paginated(&pool, 2, 4, &FilterParams::default())
        .await
        .unwrap();
    assert!(items.is_empty());
    assert_eq!(total, 3);
}

#[tokio::test]
async fn query_archived_enriched_paginates() {
    let pool = test_pool().await;
    for i in 1..=3 {
        let notif = crate::db::queries::NotificationRow {
            id: format!("a{i}"),
            pr_id: Some(100 + i),
            title: format!("Archived PR {i}"),
            repository: "owner/repo".to_string(),
            reason: "mention".to_string(),
            unread: false,
            archived: true,
            updated_at: format!("2025-02-0{i}T00:00:00Z"),
        };
        // Seed as archived: read first-contact under the cold-start policy.
        crate::db::queries::upsert_notification(&pool, &notif, 1, true)
            .await
            .unwrap();
    }
    let (items, total) = query_archived_enriched_paginated(&pool, 2, 0, &FilterParams::default())
        .await
        .unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(total, 3);
}

#[tokio::test]
async fn upsert_roundtrip_labels() {
    let pool = test_pool().await;
    let mut pr = sample(400);
    pr.labels =
        r#"[{"name":"bug","color":"d73a4a"},{"name":"enhancement","color":"a2eeef"}]"#.to_string();
    upsert_pull_request(&pool, &pr).await.unwrap();
    let row = get_pull_request(&pool, "owner/repo", 400)
        .await
        .unwrap()
        .unwrap();
    assert!(row.labels.contains("bug"));
    assert!(row.labels.contains("enhancement"));
}

/// `status` is one of "open" | "draft" | "merged" | "closed", expanded into
/// the (state, draft, merged_at) columns the same way the pr_status CASE
/// derives it back.
async fn insert_notif_with_pr(
    pool: &SqlitePool,
    notif_id: &str,
    repo: &str,
    pr_id: i64,
    author: &str,
    status: &str,
) {
    let (state, draft, merged) = match status {
        "draft" => ("open", true, false),
        "merged" => ("open", false, true),
        other => (other, false, false),
    };
    sqlx::query(
        "INSERT INTO notifications (id, title, repository, reason, unread, archived, updated_at, pr_id)
         VALUES (?, 'T', ?, 'mention', 0, 0, '2025-01-01', ?)",
    )
    .bind(notif_id)
    .bind(repo)
    .bind(pr_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO pull_requests (id, title, repo, author, url, ci_status, body, state, head_sha, additions, deletions, changed_files, draft, merged_at, labels)
         VALUES (?, 'T', ?, ?, 'https://x.com', NULL, '', ?, 'abc', 0, 0, 0, ?, ?, '[]')",
    )
    .bind(pr_id)
    .bind(repo)
    .bind(author)
    .bind(state)
    .bind(draft)
    .bind(if merged { Some("2025-01-02T00:00:00Z") } else { None::<&str> })
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn filter_by_repo() {
    let pool = test_pool().await;
    insert_notif_with_pr(&pool, "n1", "acme/api", 1, "alice", "open").await;
    insert_notif_with_pr(&pool, "n2", "acme/web", 2, "bob", "open").await;
    let f = FilterParams {
        repo: Some("acme/api".to_string()),
        ..Default::default()
    };
    let (items, total) = query_inbox_enriched_paginated(&pool, 100, 0, &f)
        .await
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(items[0].repository, "acme/api");
}

#[tokio::test]
async fn filter_by_author() {
    let pool = test_pool().await;
    insert_notif_with_pr(&pool, "n1", "acme/api", 1, "alice", "open").await;
    insert_notif_with_pr(&pool, "n2", "acme/web", 2, "bob", "open").await;
    let f = FilterParams {
        author: Some("alice".to_string()),
        ..Default::default()
    };
    let (items, total) = query_inbox_enriched_paginated(&pool, 100, 0, &f)
        .await
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(items[0].author, Some("alice".to_string()));
}

#[tokio::test]
async fn filter_by_state_open_excludes_drafts() {
    let pool = test_pool().await;
    insert_notif_with_pr(&pool, "n1", "acme/api", 1, "alice", "open").await;
    insert_notif_with_pr(&pool, "n2", "acme/web", 2, "bob", "draft").await; // draft
    let f = FilterParams {
        state_include: vec!["open".to_string()],
        ..Default::default()
    };
    let (items, total) = query_inbox_enriched_paginated(&pool, 100, 0, &f)
        .await
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(items[0].repository, "acme/api");
}

#[tokio::test]
async fn filter_by_state_merged() {
    let pool = test_pool().await;
    insert_notif_with_pr(&pool, "n1", "acme/api", 1, "alice", "merged").await; // merged
    insert_notif_with_pr(&pool, "n2", "acme/web", 2, "bob", "open").await;
    let f = FilterParams {
        state_include: vec!["merged".to_string()],
        ..Default::default()
    };
    let (items, total) = query_inbox_enriched_paginated(&pool, 100, 0, &f)
        .await
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(items[0].repository, "acme/api");
}

#[tokio::test]
async fn no_filters_returns_all() {
    let pool = test_pool().await;
    insert_notif_with_pr(&pool, "n1", "acme/api", 1, "alice", "open").await;
    insert_notif_with_pr(&pool, "n2", "acme/web", 2, "bob", "open").await;
    let (items, total) = query_inbox_enriched_paginated(&pool, 100, 0, &FilterParams::default())
        .await
        .unwrap();
    assert_eq!(total, 2);
    assert_eq!(items.len(), 2);
}

#[tokio::test]
async fn filter_by_state_draft() {
    let pool = test_pool().await;
    insert_notif_with_pr(&pool, "n1", "acme/api", 1, "alice", "draft").await; // draft
    insert_notif_with_pr(&pool, "n2", "acme/web", 2, "bob", "open").await; // open
    let f = FilterParams {
        state_include: vec!["draft".to_string()],
        ..Default::default()
    };
    let (items, total) = query_inbox_enriched_paginated(&pool, 100, 0, &f)
        .await
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(items[0].repository, "acme/api");
}

#[tokio::test]
async fn filter_by_state_closed() {
    let pool = test_pool().await;
    insert_notif_with_pr(&pool, "n1", "acme/api", 1, "alice", "closed").await; // closed
    insert_notif_with_pr(&pool, "n2", "acme/web", 2, "bob", "open").await; // open
    let f = FilterParams {
        state_include: vec!["closed".to_string()],
        ..Default::default()
    };
    let (items, total) = query_inbox_enriched_paginated(&pool, 100, 0, &f)
        .await
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(items[0].repository, "acme/api");
}

#[tokio::test]
async fn filter_includes_multiple_states() {
    let pool = test_pool().await;
    insert_notif_with_pr(&pool, "n1", "acme/api", 1, "alice", "open").await; // open
    insert_notif_with_pr(&pool, "n2", "acme/web", 2, "bob", "draft").await; // draft
    insert_notif_with_pr(&pool, "n3", "acme/cli", 3, "carol", "merged").await; // merged
    let f = FilterParams {
        state_include: vec!["open".to_string(), "draft".to_string()],
        ..Default::default()
    };
    let (items, total) = query_inbox_enriched_paginated(&pool, 100, 0, &f)
        .await
        .unwrap();
    assert_eq!(total, 2);
    let repos: Vec<&str> = items.iter().map(|i| i.repository.as_str()).collect();
    assert!(repos.contains(&"acme/api"));
    assert!(repos.contains(&"acme/web"));
}

#[tokio::test]
async fn filter_excludes_states() {
    let pool = test_pool().await;
    insert_notif_with_pr(&pool, "n1", "acme/api", 1, "alice", "open").await; // open
    insert_notif_with_pr(&pool, "n2", "acme/web", 2, "bob", "merged").await; // merged
    insert_notif_with_pr(&pool, "n3", "acme/cli", 3, "carol", "closed").await; // closed
    let f = FilterParams {
        state_exclude: vec!["merged".to_string(), "closed".to_string()],
        ..Default::default()
    };
    let (items, total) = query_inbox_enriched_paginated(&pool, 100, 0, &f)
        .await
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(items[0].repository, "acme/api");
}

#[tokio::test]
async fn filter_by_team() {
    let pool = test_pool().await;
    // Insert n1 with pr_id=1, teams=["acme/platform"]
    sqlx::query(
        "INSERT INTO notifications (id, title, repository, reason, unread, archived, updated_at, pr_id)
         VALUES ('n1', 'T', 'acme/api', 'mention', 0, 0, '2025-01-01', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO pull_requests (id, title, repo, author, url, ci_status, body, state, head_sha, additions, deletions, changed_files, draft, labels, teams)
         VALUES (1, 'T', 'acme/api', 'alice', 'https://x.com', NULL, '', 'open', 'abc', 0, 0, 0, 0, '[]', '[\"acme/platform\"]')",
    )
    .execute(&pool)
    .await
    .unwrap();
    // Insert n2 with pr_id=2, no teams
    sqlx::query(
        "INSERT INTO notifications (id, title, repository, reason, unread, archived, updated_at, pr_id)
         VALUES ('n2', 'T', 'acme/web', 'mention', 0, 0, '2025-01-01', 2)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO pull_requests (id, title, repo, author, url, ci_status, body, state, head_sha, additions, deletions, changed_files, draft, labels)
         VALUES (2, 'T', 'acme/web', 'bob', 'https://x.com', NULL, '', 'open', 'abc', 0, 0, 0, 0, '[]')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Should match the PR with acme/platform team
    let f = FilterParams {
        team: Some("acme/platform".to_string()),
        ..Default::default()
    };
    let (items, total) = query_inbox_enriched_paginated(&pool, 100, 0, &f)
        .await
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(items[0].repository, "acme/api");

    // Should NOT match a different team
    let f2 = FilterParams {
        team: Some("acme/backend".to_string()),
        ..Default::default()
    };
    let (items2, total2) = query_inbox_enriched_paginated(&pool, 100, 0, &f2)
        .await
        .unwrap();
    assert_eq!(total2, 0);
    assert!(items2.is_empty());
}

#[tokio::test]
async fn update_ci_status_sets_clears_and_ignores_wrong_repo() {
    let pool = test_pool().await;
    let mut pr = sample(42);
    pr.ci_status = None;
    upsert_pull_request(&pool, &pr).await.unwrap();

    // Set a value
    update_ci_status(&pool, 42, "owner/repo", Some("pending"))
        .await
        .unwrap();
    let row = get_pull_request(&pool, "owner/repo", 42)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.ci_status.as_deref(), Some("pending"));

    // Update to a different value
    update_ci_status(&pool, 42, "owner/repo", Some("success"))
        .await
        .unwrap();
    let row = get_pull_request(&pool, "owner/repo", 42)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.ci_status.as_deref(), Some("success"));

    // Clear it
    update_ci_status(&pool, 42, "owner/repo", None)
        .await
        .unwrap();
    let row = get_pull_request(&pool, "owner/repo", 42)
        .await
        .unwrap()
        .unwrap();
    assert!(row.ci_status.is_none());

    // Wrong repo — should not affect the row
    update_ci_status(&pool, 42, "other/repo", Some("failure"))
        .await
        .unwrap();
    let row = get_pull_request(&pool, "owner/repo", 42)
        .await
        .unwrap()
        .unwrap();
    assert!(row.ci_status.is_none());
}
