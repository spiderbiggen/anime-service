use crate::{models, Error, HYPER};
use itertools::Itertools;
use rocket::serde::json::Json;
use std::result::Result;

#[get("/<title>")]
pub(crate) async fn get(title: &str) -> Result<Json<Vec<models::DirectDownload>>, Error> {
    let episodes = nyaa::downloads(HYPER.clone(), title).await?;
    let result = episodes.into_iter().map(|e| e.into()).collect();
    Ok(Json(result))
}

#[get("/<title>/groups")]
pub(crate) async fn get_groups(title: &str) -> Result<Json<Vec<models::DownloadGroup>>, Error> {
    let episodes = nyaa::groups(HYPER.clone(), title).await?;
    let result = episodes
        .into_iter()
        .map(|e| Into::into(e))
        .sorted_by_key(|a: &models::DownloadGroup| a.episode.episode)
        .rev()
        .collect();
    Ok(Json(result))
}
