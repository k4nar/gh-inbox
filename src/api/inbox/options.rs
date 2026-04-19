use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;

use crate::api::AppError;
use crate::db::queries;
use crate::server::AppState;

#[derive(Deserialize)]
pub struct OptionsQuery {
    pub status: Option<String>,
}

/// GET /api/inbox/options — return available filter options.
/// Pass `?status=archived` to get options scoped to the last 200 archived notifications.
pub async fn get_inbox_options(
    State(state): State<AppState>,
    Query(query): Query<OptionsQuery>,
) -> Result<Json<queries::InboxOptions>, AppError> {
    let archived = query.status.as_deref() == Some("archived");
    let options = queries::get_inbox_options(&state.pool, archived).await?;
    Ok(Json(options))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn no_query() -> axum::extract::Query<OptionsQuery> {
        axum::extract::Query(OptionsQuery { status: None })
    }

    #[tokio::test]
    async fn returns_empty_options_for_empty_db() {
        let pool = crate::db::init_with_path(":memory:").await;
        let (_, state) = crate::server::app(pool, Arc::from("token"));

        let result = get_inbox_options(axum::extract::State(state), no_query()).await;
        assert!(result.is_ok());
        let Json(options) = result.unwrap();
        assert!(options.repos.is_empty());
        assert!(options.orgs.is_empty());
        assert!(options.teams.is_empty());
        assert!(options.authors.is_empty());
    }

    #[tokio::test]
    async fn returns_options_shape_with_data() {
        let pool = crate::db::init_with_path(":memory:").await;
        sqlx::query(
            "INSERT INTO notifications (id, title, repository, reason, unread, archived, updated_at)
             VALUES ('n1', 'T', 'acme/api', 'mention', 0, 0, '2025-01-01')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let (_, state) = crate::server::app(pool, Arc::from("token"));

        let result = get_inbox_options(axum::extract::State(state), no_query()).await;
        assert!(result.is_ok());
        let Json(options) = result.unwrap();
        assert_eq!(options.repos, vec!["acme/api"]);
        assert_eq!(options.orgs, vec!["acme"]);
    }
}
