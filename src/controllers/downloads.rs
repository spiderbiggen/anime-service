use rocket::serde::json::Json;
use std::result::Result;
use crate::{models, Error, HYPER};

#[get("/<title>")]
pub(crate) async fn get(title: &str) -> Result<Json<Vec<models::DirectDownload>>, Error> {
    let episodes = nyaa::downloads(HYPER.clone(), title).await?;
    let result = episodes.into_iter().map(|e| e.into()).collect();
    Ok(Json(result))
}

#[get("/<title>/groups")]
pub(crate) async fn get_groups(title: &str) -> Result<Json<Vec<models::DownloadGroup>>, Error> {
    let episodes = nyaa::groups(HYPER.clone(), title).await?;
    let result = episodes.into_iter().map(|e| e.into()).collect();
    Ok(Json(result))
}
