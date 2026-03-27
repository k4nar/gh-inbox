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
}

#[derive(serde::Serialize)]
pub struct PaginatedInbox {
    pub items: Vec<InboxItem>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
}

/// GET /api/inbox — return enriched notifications.
pub async fn get_inbox(
    State(state): State<AppState>,
    Query(query): Query<InboxQuery>,
) -> Result<Json<PaginatedInbox>, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(25).clamp(1, 100);
    let offset = (page - 1) * per_page;

    let (items, total) = match query.status.as_deref() {
        Some("archived") => {
            queries::query_archived_enriched_paginated(&state.pool, per_page, offset).await?
        }
        _ => queries::query_inbox_enriched_paginated(&state.pool, per_page, offset).await?,
    };

    Ok(Json(PaginatedInbox {
        items,
        total,
        page,
        per_page,
    }))
}
