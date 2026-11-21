//! Deserialization tests against hand-written fixtures that follow TMDB's documented response
//! shapes. They are not captured responses, so the ids and texts are made up.

use jiff::civil::date;
use tmdb::ListResponse;
use tmdb::search::TvSearchResult;
use tmdb::tv::{AlternativeTitle, ExternalIds, TvDetails, romaji_title};

#[test]
fn parses_a_full_search_result() {
    let page: ListResponse<TvSearchResult> =
        serde_json::from_str(include_str!("fixtures/search_tv.json")).expect("fixture parses");

    assert_eq!(page.page, 1);
    assert_eq!(page.total_pages, 3);
    assert_eq!(page.total_results, 42);
    assert_eq!(page.results.len(), 3);

    let show = &page.results[0];
    assert_eq!(show.id, 1001);
    assert_eq!(show.name, "Sample Anime");
    assert_eq!(show.original_name, "サンプルアニメ");
    assert_eq!(show.first_air_date, Some(date(2023, 9, 29)));
    assert_eq!(show.poster_path.as_deref(), Some("/sample-poster.jpg"));
    assert_eq!(show.origin_country, ["JP"]);
    assert!(show.is_animation());
}

#[test]
fn null_and_empty_values_become_none() {
    let page: ListResponse<TvSearchResult> =
        serde_json::from_str(include_str!("fixtures/search_tv.json")).expect("fixture parses");

    let show = &page.results[1];
    assert_eq!(show.first_air_date, None);
    assert_eq!(show.poster_path, None);
    assert_eq!(show.backdrop_path, None);
    assert_eq!(show.overview, "");
    assert!(!show.is_animation());
}

#[test]
fn missing_optional_fields_fall_back_to_defaults() {
    let page: ListResponse<TvSearchResult> =
        serde_json::from_str(include_str!("fixtures/search_tv.json")).expect("fixture parses");

    let show = &page.results[2];
    assert_eq!(show.id, 1003);
    assert_eq!(show.name, "Sparse Entry");
    assert_eq!(show.original_name, "");
    assert_eq!(show.genre_ids, Vec::<u32>::new());
    assert_eq!(show.first_air_date, None);
    assert_eq!(show.vote_count, 0);
}

#[test]
fn parses_tv_details() {
    let details: TvDetails =
        serde_json::from_str(include_str!("fixtures/tv_details.json")).expect("fixture parses");

    assert_eq!(details.id, 1001);
    assert_eq!(details.status.as_deref(), Some("Ended"));
    assert_eq!(details.first_air_date, Some(date(2023, 9, 29)));
    assert_eq!(details.last_air_date, Some(date(2024, 3, 22)));
    assert_eq!(details.number_of_episodes, Some(28));
    assert_eq!(details.number_of_seasons, Some(1));
    assert_eq!(details.genres.len(), 2);
    assert_eq!(details.genres[0].name, "Animation");
    assert_eq!(
        details.backdrop_path.as_deref(),
        Some("/sample-backdrop.jpg")
    );
}

#[test]
fn parses_external_ids() {
    let ids: ExternalIds =
        serde_json::from_str(include_str!("fixtures/external_ids.json")).expect("fixture parses");

    assert_eq!(ids.id, 1001);
    assert_eq!(ids.tvdb_id, Some(424_242));
    assert_eq!(ids.imdb_id.as_deref(), Some("tt0000001"));
}

#[test]
fn external_ids_tolerate_null_and_empty_values() {
    let ids: ExternalIds =
        serde_json::from_str(r#"{ "id": 5, "imdb_id": "", "tvdb_id": null }"#).expect("parses");

    assert_eq!(ids.tvdb_id, None);
    assert_eq!(ids.imdb_id, None);
}

#[test]
fn an_invalid_date_is_an_error() {
    let result = serde_json::from_str::<TvSearchResult>(
        r#"{ "id": 1, "name": "x", "first_air_date": "not-a-date" }"#,
    );

    assert!(result.is_err());
}

fn alternative_titles() -> Vec<AlternativeTitle> {
    #[derive(serde::Deserialize)]
    struct Response {
        results: Vec<AlternativeTitle>,
    }
    serde_json::from_str::<Response>(include_str!("fixtures/alternative_titles.json"))
        .expect("fixture parses")
        .results
}

#[test]
fn parses_alternative_titles() {
    let titles = alternative_titles();

    assert_eq!(titles.len(), 5);
    assert_eq!(titles[0].title, "示例动画");
    assert_eq!(titles[0].kind, "");
    assert_eq!(titles[2].kind, "Hepburn Romanization");
}

#[test]
fn the_romaji_title_ignores_case_and_other_romanizations() {
    // The fixture has a Hepburn romanization before the `romaji` entry.
    assert_eq!(
        romaji_title(&alternative_titles()),
        Some("Sanpuru no Anime")
    );

    let upper: Vec<AlternativeTitle> =
        serde_json::from_str(r#"[{ "iso_3166_1": "JP", "title": "Upper", "type": "ROMAJI" }]"#)
            .expect("parses");
    assert_eq!(romaji_title(&upper), Some("Upper"));
}

#[test]
fn no_romaji_title_when_there_is_none() {
    let titles: Vec<AlternativeTitle> = serde_json::from_str(
        r#"[
            { "iso_3166_1": "JP", "title": "Hepburn", "type": "Hepburn Romanization" },
            { "iso_3166_1": "JP", "title": "", "type": "Romaji" },
            { "iso_3166_1": "US", "title": "English" }
        ]"#,
    )
    .expect("parses");

    assert_eq!(romaji_title(&titles), None);
    assert_eq!(romaji_title(&[]), None);
}
