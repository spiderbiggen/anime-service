use std::collections::BTreeMap;
use chrono::{Utc, DateTime};
use serde_json::Value;
use url::Url;

#[derive(Deserialize, Clone, Debug)]
pub struct LinkRel {
    #[serde(rename = "self")]
    pub this: Url,
    pub related: Option<Url>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LinkWrapper {
    pub links: LinkRel,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Relationships {
    pub genres: LinkWrapper,
    pub categories: LinkWrapper,
    pub castings: LinkWrapper,
    pub installments: LinkWrapper,
    pub mappings: LinkWrapper,
    pub reviews: LinkWrapper,
    pub media_relationships: LinkWrapper,
    pub characters: LinkWrapper,
    pub staff: LinkWrapper,
    pub productions: LinkWrapper,
    pub quotes: LinkWrapper,
    pub episodes: LinkWrapper,
    pub streaming_links: LinkWrapper,
    pub anime_productions: LinkWrapper,
    pub anime_characters: LinkWrapper,
    pub anime_staff: LinkWrapper,
}

#[derive(Deserialize, Copy, Clone, Debug)]
pub struct ImageMeta {
    pub dimensions: ImageDimensions,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Images {
    pub original: Url,
    pub large: Option<Url>,
    pub medium: Option<Url>,
    pub small: Option<Url>,
    pub tiny: Option<Url>,
    pub meta: ImageMeta,
}

#[derive(Deserialize, Copy, Clone, Debug)]
pub struct ImageDimension {
    pub width: u32,
    pub height: u32,
}

#[derive(Deserialize, Copy, Clone, Debug)]
pub struct ImageDimensions {
    pub large: Option<ImageDimension>,
    pub medium: Option<ImageDimension>,
    pub small: Option<ImageDimension>,
    pub tiny: Option<ImageDimension>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Titles {
    pub en: String,
    pub en_jp: String,
    pub ja_jp: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Attributes {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub slug: String,
    pub synopsis: String,
    pub description: String,
    pub cover_image_top_offset: u32,
    pub titles: Titles,
    pub canonical_title: String,
    pub abbreviated_titles: Vec<String>,
    pub average_rating: String,
    pub rating_frequencies: BTreeMap<u32, String>,
    pub user_count: u32,
    pub favorites_count: u32,
    pub start_date: String,
    pub end_date: String,
    pub next_release: Value,
    // TODO when api provides a value
    pub popularity_rank: u32,
    pub rating_rank: u32,
    pub age_rating: String,
    pub age_rating_guide: String,
    pub subtype: String,
    pub status: String,
    pub tba: Value,
    // TODO when api provides a value
    pub poster_image: Images,
    pub cover_image: Images,
    pub episode_count: u32,
    pub episode_length: u32,
    pub total_length: Option<u64>,
    pub youtube_video_id: Option<String>,
    pub nsfw: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Anime {
    pub id: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub links: LinkRel,
    pub attributes: Attributes,
    pub relationships: Relationships,
}