use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;
use tracing::error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("internal_server_error: {0}")]
    InternalServerError(#[from] anyhow::Error),
    #[error("authorization_error")]
    AuthorizationError,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            Self::InternalServerError(inner) => {
                error!(errror = ?inner, "handler failed to process request");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_server_error",
                    #[cfg(debug_assertions)]
                    format!("something went wrong:\n{inner}"),
                    #[cfg(not(debug_assertions))]
                    "something went wrong".to_owned(),
                )
            }
            Self::AuthorizationError => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "authorization failed".to_owned(),
            ),
        };

        let body = Json(json!({
            "error_code": code,
            "message": message,
        }));
        (status, body).into_response()
    }
}
