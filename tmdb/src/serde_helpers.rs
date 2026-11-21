//! Deserializers for TMDB's habit of sending empty strings instead of `null`.

use jiff::civil::Date;
use serde::de::Error;
use serde::{Deserialize, Deserializer};

/// Deserializes `null`, a missing value or `""` as `None`, and any other string as a date.
///
/// A non-empty string that is not a valid `YYYY-MM-DD` date is an error.
pub(crate) fn optional_date<'de, D>(deserializer: D) -> Result<Option<Date>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<String>::deserialize(deserializer)?.as_deref() {
        None | Some("") => Ok(None),
        Some(value) => value.parse().map(Some).map_err(Error::custom),
    }
}

/// Deserializes `null`, a missing value or `""` as `None`.
pub(crate) fn optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.filter(|value| !value.is_empty()))
}
