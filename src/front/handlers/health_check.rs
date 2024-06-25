use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::app_error::AppError;

pub async fn health_check() -> Result<impl IntoResponse, AppError> {
    Ok((
        StatusCode::OK,
        Json(json!({
            "status": "ok",
        })),
    ))
}
