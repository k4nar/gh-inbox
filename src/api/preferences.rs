use axum::extract::State;
use axum::http::StatusCode;
use serde_json::{Map, Value};

use crate::api::AppError;
use crate::db::queries;
use crate::server::AppState;

const VALID_THEMES: &[&str] = &[
    "system",
    "light",
    "dark",
    "catppuccin-latte",
    "catppuccin-frappe",
    "catppuccin-macchiato",
    "catppuccin-mocha",
];

pub async fn get_preferences(State(state): State<AppState>) -> Result<axum::Json<Value>, AppError> {
    let theme = queries::get_preference(&state.pool, "theme")
        .await?
        .unwrap_or_else(|| "system".to_string());
    Ok(axum::Json(serde_json::json!({ "theme": theme })))
}

pub async fn patch_preferences(
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> Result<StatusCode, AppError> {
    let map: Map<String, Value> = serde_json::from_slice(&body)
        .map_err(|e| AppError::BadRequest(format!("invalid JSON: {e}")))?;

    for (key, value) in &map {
        let v = value
            .as_str()
            .ok_or_else(|| AppError::BadRequest(format!("value for '{key}' must be a string")))?;

        match key.as_str() {
            "theme" if VALID_THEMES.contains(&v) => {}
            "theme" => {
                return Err(AppError::BadRequest(format!(
                    "invalid theme '{v}'; valid values: {}",
                    VALID_THEMES.join(", ")
                )));
            }
            _ => {
                return Err(AppError::BadRequest(format!(
                    "unknown preference key '{key}'"
                )));
            }
        }

        queries::upsert_preference(&state.pool, key, v).await?;
    }

    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> axum::Router<AppState> {
    use axum::routing::get;
    axum::Router::new().route(
        "/api/preferences",
        get(get_preferences).patch(patch_preferences),
    )
}
