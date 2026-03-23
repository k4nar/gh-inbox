use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::github;

/// Typed error for API handlers. Maps to appropriate HTTP status codes.
#[derive(Debug)]
pub enum AppError {
    /// GitHub API returned an error.
    GitHub(reqwest::Error),
    /// Database query failed.
    Database(sqlx::Error),
    /// Resource not found.
    NotFound(String),
    /// Internal server error.
    Internal(String),
    /// Client sent an invalid request.
    BadRequest(String),
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::GitHub(err)
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Database(err)
    }
}

impl From<github::sync::SyncError> for AppError {
    fn from(err: github::sync::SyncError) -> Self {
        use github::sync::SyncError;
        match err {
            SyncError::GitHub(e) => AppError::GitHub(e),
            SyncError::Database(e) => AppError::Database(e),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::GitHub(err) => {
                let status = err
                    .status()
                    .map(|s| StatusCode::from_u16(s.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY))
                    .unwrap_or(StatusCode::BAD_GATEWAY);
                (status, format!("GitHub API error: {err}"))
            }
            AppError::Database(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database error: {err}"),
            ),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Internal(msg) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
        };

        tracing::warn!("{message}");
        (status, message).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn github_network_error_maps_to_bad_gateway() {
        // Trigger a real reqwest error by connecting to a port nothing listens on
        let err = reqwest::Client::new()
            .get("http://127.0.0.1:1/bad")
            .send()
            .await
            .unwrap_err();
        let app_err = AppError::GitHub(err);
        let response = app_err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn not_found_error_maps_to_404() {
        let app_err = AppError::NotFound("notification not found".to_string());
        let response = app_err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn database_error_maps_to_500() {
        let err = sqlx::Error::RowNotFound;
        let app_err = AppError::Database(err);
        let response = app_err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn bad_request_error_maps_to_400() {
        let app_err = AppError::BadRequest("invalid theme".to_string());
        let response = app_err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
