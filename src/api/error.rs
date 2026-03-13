use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// Typed error for API handlers. Maps to appropriate HTTP status codes.
#[derive(Debug)]
pub enum AppError {
    /// GitHub API returned an error.
    GitHub(reqwest::Error),
    /// Database query failed.
    Database(sqlx::Error),
    /// Resource not found.
    NotFound(String),
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

impl From<crate::github::sync::SyncError> for AppError {
    fn from(err: crate::github::sync::SyncError) -> Self {
        match err {
            crate::github::sync::SyncError::GitHub(e) => AppError::GitHub(e),
            crate::github::sync::SyncError::Database(e) => AppError::Database(e),
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
        };

        eprintln!("{message}");
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
}
