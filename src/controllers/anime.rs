use std::result::Result;
use axum::{Json, extract::Path};
use std::num::ParseIntError;

use crate::{models, Error, HYPER};

pub async fn get_single(Path(id): Path<u32>) -> Result<Json<models::Show>, Error> {
    let anime = kitsu::anime::single(HYPER.clone(), id).await?;
    let show = anime.data.try_into()?;
    Ok(Json(show))
}

pub async fn get_collection() -> Result<Json<Vec<models::Show>>, Error> {
    let anime = kitsu::anime::collection(HYPER.clone()).await?;
    let show: Result<Vec<models::Show>, ParseIntError> = anime.data.into_iter().map(|d| d.try_into()).collect();
    Ok(Json(show?))
}
