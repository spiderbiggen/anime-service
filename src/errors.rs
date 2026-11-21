use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use tracing::error;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Tmdb(#[from] tmdb::Error),
    #[error(transparent)]
    Nyaa(#[from] nyaa::Error),
    #[error(transparent)]
    Internal(#[from] InternalError),
}

impl Error {
    /// The status to answer with. Upstream failures are mapped so that clients can tell "not
    /// found" and "slow down" apart from "the upstream service is broken", without mistaking an
    /// upstream auth failure for their own.
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Nyaa(nyaa::Error::Status(code)) => *code,
            Self::Tmdb(tmdb::Error::Status(code)) => match *code {
                StatusCode::NOT_FOUND | StatusCode::TOO_MANY_REQUESTS => *code,
                _ => StatusCode::BAD_GATEWAY,
            },
            Self::Tmdb(_) => StatusCode::BAD_GATEWAY,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        error!("request failed with {self}");
        let status = self.status_code();
        let body = Json(json!({
            "error": status.canonical_reason().unwrap_or_default(),
        }));
        (status, body).into_response()
    }
}

pub type InternalError = anyhow::Error;

#[cfg(test)]
mod tests {
    use super::*;

    fn tmdb_status(code: StatusCode) -> Error {
        Error::Tmdb(tmdb::Error::Status(code))
    }

    #[test]
    fn tmdb_not_found_and_rate_limits_pass_through() {
        assert_eq!(
            tmdb_status(StatusCode::NOT_FOUND).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            tmdb_status(StatusCode::TOO_MANY_REQUESTS).status_code(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[test]
    fn other_tmdb_failures_are_a_bad_gateway() {
        for code in [
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::INTERNAL_SERVER_ERROR,
        ] {
            assert_eq!(tmdb_status(code).status_code(), StatusCode::BAD_GATEWAY);
        }
    }

    #[test]
    fn nyaa_statuses_pass_through() {
        let error = Error::Nyaa(nyaa::Error::Status(StatusCode::SERVICE_UNAVAILABLE));
        assert_eq!(error.status_code(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn everything_else_is_an_internal_error() {
        let error = Error::Internal(anyhow::anyhow!("boom"));
        assert_eq!(error.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
