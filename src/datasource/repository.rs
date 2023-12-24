use chrono::{DateTime, Utc};
use sqlx::types::Uuid;

pub mod download;
pub mod download_resolution;

// TODO: remove default and provide dynamically
const PROVIDER_DEFAULT: &str = "SubsPlease";

#[derive(Debug, Copy, Clone, sqlx::Type)]
#[sqlx(type_name = "download_variant", rename_all = "lowercase")]
pub enum Variant {
    Batch,
    Episode,
    Movie,
}

struct RawSingleDownloadResult {
    id: Uuid,
    updated_at: DateTime<Utc>,
    resolutions: Option<Vec<i16>>,
}

struct SingleDownloadResult {
    id: Uuid,
    updated_at: DateTime<Utc>,
    resolutions: Vec<u16>,
}

impl From<RawSingleDownloadResult> for SingleDownloadResult {
    fn from(value: RawSingleDownloadResult) -> Self {
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
