use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;

use crate::api::AppError;
use crate::db::queries::{self, InboxItem};
use crate::server::AppState;

#[derive(Deserialize)]
pub struct InboxQuery {
    pub status: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub repo: Option<String>,
    pub team: Option<String>,
    pub author: Option<String>,
    /// Comma-separated PR statuses to show exclusively (e.g. "open,draft").
    pub state_include: Option<String>,
    /// Comma-separated PR statuses to hide (e.g. "merged,closed").
    pub state_exclude: Option<String>,
}

/// Split a comma-separated query value into a list, dropping empty entries.
fn parse_csv(value: Option<String>) -> Vec<String> {
    value
        .map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[derive(serde::Serialize)]
pub struct PaginatedInbox {
    pub items: Vec<InboxItem>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
}

/// GET /api/inbox — return enriched notifications with optional server-side filtering.
pub async fn get_inbox(
    State(state): State<AppState>,
    Query(query): Query<InboxQuery>,
) -> Result<Json<PaginatedInbox>, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(25).clamp(1, 100);
    let offset = (page - 1) * per_page;

    let filters = queries::FilterParams {
        repo: query.repo,
        team: query.team,
        author: query.author,
        state_include: parse_csv(query.state_include),
        state_exclude: parse_csv(query.state_exclude),
    };

    let (items, total) = match query.status.as_deref() {
        Some("archived") => {
            queries::query_archived_enriched_paginated(&state.pool, per_page, offset, &filters)
                .await?
        }
        _ => {
            queries::query_inbox_enriched_paginated(&state.pool, per_page, offset, &filters).await?
        }
    };

    Ok(Json(PaginatedInbox {
        items,
        total,
        page,
        per_page,
    }))
}

#[cfg(test)]
mod tests {
    use axum::http::{Method, StatusCode};
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tower::util::ServiceExt;

    async fn server_with_two_prs() -> axum::Router {
        let pool = crate::db::init_with_path(":memory:").await;
        for (notif_id, repo, pr_id, author) in [
            ("n1", "acme/api", 1_i64, "alice"),
            ("n2", "beta/app", 2_i64, "bob"),
        ] {
            sqlx::query(
                "INSERT INTO notifications (id, title, repository, reason, unread, archived, updated_at, pr_id)
                 VALUES (?, 'T', ?, 'mention', 0, 0, '2025-01-01', ?)",
            )
            .bind(notif_id)
            .bind(repo)
            .bind(pr_id)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO pull_requests (id, title, repo, author, url, ci_status, body, state, head_sha, additions, deletions, changed_files, draft, labels)
                 VALUES (?, 'T', ?, ?, 'https://x.com', NULL, '', 'open', 'abc', 0, 0, 0, 0, '[]')",
            )
            .bind(pr_id)
            .bind(repo)
            .bind(author)
            .execute(&pool)
            .await
            .unwrap();
        }
        let (app, _) = crate::app(pool, Arc::from("token"));
        app
    }

    #[tokio::test]
    async fn filter_by_repo_returns_one_item() {
        let app = server_with_two_prs().await;
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method(Method::GET)
                    .uri("/api/inbox?repo=acme%2Fapi")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let data: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(data["total"], 1);
        assert_eq!(data["items"][0]["repository"], "acme/api");
    }

    #[tokio::test]
    async fn filter_by_author_returns_one_item() {
        let app = server_with_two_prs().await;
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method(Method::GET)
                    .uri("/api/inbox?author=alice")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let data: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(data["total"], 1);
        assert_eq!(data["items"][0]["author"], "alice");
    }

    #[tokio::test]
    async fn no_filter_returns_all_items() {
        let app = server_with_two_prs().await;
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method(Method::GET)
                    .uri("/api/inbox")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let data: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(data["total"], 2);
    }
}
