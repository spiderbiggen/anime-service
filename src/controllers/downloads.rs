use crate::{models, Error, HyperClient};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::{extract::Query, response::Response, Json};
use itertools::Itertools;
use serde::Deserialize;
use std::result::Result;

#[derive(Debug, Deserialize)]
pub(crate) struct DownloadQuery {
    title: Option<String>,
    grouped: Option<bool>,
}

pub(crate) async fn get(
    Query(params): Query<DownloadQuery>,
    State(hyper): State<HyperClient>,
) -> Response {
    let title = params.title.unwrap_or("".to_owned());
    match params.grouped {
        Some(true) => match get_groups(hyper, title).await {
            Ok(groups) => groups.into_response(),
            Err(e) => e.into_response(),
        },
        _ => match get_ungrouped(hyper, title).await {
            Ok(groups) => groups.into_response(),
            Err(e) => e.into_response(),
        },
    }
}

pub(crate) async fn get_ungrouped(
    hyper: HyperClient,
    title: String,
) -> Result<Json<Vec<models::DirectDownload>>, Error> {
    let result = nyaa::downloads(hyper, &title)
        .await?
        .into_iter()
        .map(|e| e.into())
        .sorted_by_key(|a: &models::DirectDownload| a.pub_date)
        .rev()
        .collect();
    Ok(Json(result))
}

pub(crate) async fn get_groups(
    hyper: HyperClient,
    title: String,
) -> Result<Json<Vec<models::DownloadGroup>>, Error> {
    let result = nyaa::groups(hyper, &title)
        .await?
        .into_iter()
        .map(|e| e.into())
        .sorted_by_key(|a: &models::DownloadGroup| a.episode.pub_date)
        .rev()
        .collect();
    Ok(Json(result))
}
