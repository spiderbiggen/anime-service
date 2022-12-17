use axum::extract::State;
use axum::{extract::Path, Json};
use std::num::ParseIntError;
use std::result::Result;

use crate::errors::Error;
use crate::{models, HyperClient};

pub async fn get_single(
    Path(id): Path<u32>,
    State(hyper): State<HyperClient>,
) -> Result<Json<models::Show>, Error> {
    let anime = kitsu::anime::single(hyper, id).await?;
    let show = anime.data.try_into()?;
    Ok(Json(show))
}

pub async fn get_collection(
    State(hyper): State<HyperClient>,
) -> Result<Json<Vec<models::Show>>, Error> {
    let anime = kitsu::anime::collection(hyper).await?;
    let show: Result<Vec<models::Show>, ParseIntError> =
        anime.data.into_iter().map(|d| d.try_into()).collect();
    Ok(Json(show?))
}
