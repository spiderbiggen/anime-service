use crate::{models, Error, HYPER};
use axum::{extract::Query, Json, response::Response};
use hyper::StatusCode;
use itertools::Itertools;
use serde::Deserialize;
use std::result::Result;
use axum::response::IntoResponse;

#[derive(Debug, Deserialize)]
pub(crate) struct DownloadQuery {
    title: Option<String>,
    grouped: Option<bool>,
}

pub(crate) async fn get(Query(params): Query<DownloadQuery>) -> Response {
    let title = params.title.unwrap_or("".to_owned());
    match params.grouped {
        Some(true) => match get_groups(title).await {
            Ok(groups) => groups.into_response(),
            Err(e) => e.into_response(),
        },
        _ => match get_ungrouped(title).await {
            Ok(groups) => groups.into_response(),
            Err(e) => e.into_response(),
        }
    }
}

pub(crate) async fn get_ungrouped(
    title: String,
) -> Result<Json<Vec<models::DirectDownload>>, Error> {
    let episodes = nyaa::downloads(HYPER.clone(), &title).await?;
    let result = episodes
        .into_iter()
        .map(|e| e.into())
        .sorted_by_key(|a: &models::DirectDownload| a.pub_date)
        .rev()
        .collect();
    Ok(Json(result))
}

pub(crate) async fn get_groups(title: String) -> Result<Json<Vec<models::DownloadGroup>>, Error> {
    let episodes = nyaa::groups(HYPER.clone(), &title).await?;
    let result = episodes
        .into_iter()
        .map(|e| Into::into(e))
        .sorted_by_key(|a: &models::DownloadGroup| a.episode.episode)
        .rev()
        .collect();
    Ok(Json(result))
}
