use std::num::ParseIntError;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
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
        let status = match self {
            // TODO remove unnecessary conversion between status codes once tonic upgrades to hyper 1.0
            Self::Nyaa(nyaa::Error::Status(code)) => {
                StatusCode::from_u16(code.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            }
            Self::Kitsu(kitsu::Error::Status(code)) => {
                StatusCode::from_u16(code.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = Json(json!({
            "error": status.canonical_reason().unwrap_or_default(),
        }));
        (status, body).into_response()
    }
}

pub type InternalError = anyhow::Error;
