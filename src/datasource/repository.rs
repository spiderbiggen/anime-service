use chrono::{DateTime, Utc};
use sqlx::types::Uuid;

pub mod batch;
pub mod episode;
pub mod movie;

mod download;
pub mod groups;

// TODO: remove default and provide dynamically
const PROVIDER_DEFAULT: &str = "SubsPlease";

struct RawSingleResult {
    id: Uuid,
    updated_at: DateTime<Utc>,
    resolutions: Option<Vec<i16>>,
}

struct SingleResult {
    id: Uuid,
    updated_at: DateTime<Utc>,
    resolutions: Vec<u16>,
}

impl From<RawSingleResult> for SingleResult {
    fn from(value: RawSingleResult) -> Self {
        Self {
            id: value.id,
            updated_at: value.updated_at,
            resolutions: value
                .resolutions
                .into_iter()
                .flatten()
                .map(|res| res as u16)
                .collect(),
        }
    }
}
