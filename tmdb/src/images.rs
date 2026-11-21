//! Builders for TMDB image URLs.
//!
//! TMDB returns image paths such as `/abc123.jpg`; the full URL is the image base URL, a size and
//! the path. The base URL is documented as stable, so it is a constant here instead of being
//! fetched from the `/configuration` endpoint.

/// The base URL for TMDB images.
pub const IMAGE_BASE_URL: &str = "https://image.tmdb.org/t/p/";

/// The available poster sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum PosterSize {
    W92,
    W154,
    W185,
    W342,
    W500,
    W780,
    Original,
}

impl PosterSize {
    fn as_str(self) -> &'static str {
        match self {
            Self::W92 => "w92",
            Self::W154 => "w154",
            Self::W185 => "w185",
            Self::W342 => "w342",
            Self::W500 => "w500",
            Self::W780 => "w780",
            Self::Original => "original",
        }
    }
}

/// The available backdrop sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum BackdropSize {
    W300,
    W780,
    W1280,
    Original,
}

impl BackdropSize {
    fn as_str(self) -> &'static str {
        match self {
            Self::W300 => "w300",
            Self::W780 => "w780",
            Self::W1280 => "w1280",
            Self::Original => "original",
        }
    }
}

fn image_url(size: &str, path: &str) -> String {
    format!("{IMAGE_BASE_URL}{size}/{}", path.trim_start_matches('/'))
}

/// Builds the URL of a poster image from a `poster_path`.
#[must_use]
pub fn poster_url(path: &str, size: PosterSize) -> String {
    image_url(size.as_str(), path)
}

/// Builds the URL of a backdrop image from a `backdrop_path`.
#[must_use]
pub fn backdrop_url(path: &str, size: BackdropSize) -> String {
    image_url(size.as_str(), path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_poster_urls() {
        assert_eq!(
            poster_url("/abc.jpg", PosterSize::W500),
            "https://image.tmdb.org/t/p/w500/abc.jpg"
        );
    }

    #[test]
    fn builds_backdrop_urls() {
        assert_eq!(
            backdrop_url("/abc.jpg", BackdropSize::Original),
            "https://image.tmdb.org/t/p/original/abc.jpg"
        );
    }

    #[test]
    fn tolerates_a_path_without_a_leading_slash() {
        assert_eq!(
            poster_url("abc.jpg", PosterSize::W92),
            "https://image.tmdb.org/t/p/w92/abc.jpg"
        );
    }
}
