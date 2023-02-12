use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use std::num::ParseIntError;
use tracing::error;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Kitsu(#[from] kitsu::Error),
    #[error(transparent)]
    Nyaa(#[from] nyaa::Error),
    #[error(transparent)]
    ParseInt(#[from] ParseIntError),
    #[error(transparent)]
    Internal(#[from] InternalError),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        error!("request failed with {self}");
        let (status, error_message) = match self {
            Self::Nyaa(nyaa::Error::Status(code)) => {
                (code, code.canonical_reason().unwrap_or_default())
            }
            Self::Kitsu(kitsu::Error::Status(code)) => {
                (code, code.canonical_reason().unwrap_or_default())
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal server error"),
        };
        let body = Json(json!({
            "error": error_message,
        }));
        (status, body).into_response()
    }
}

pub type InternalError = anyhow::Error;
