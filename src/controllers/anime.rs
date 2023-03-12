use std::result::Result;

use axum::extract::State;
use axum::{extract::Path, Json};

use crate::errors::Error;
use crate::models;
use crate::state::ReqwestClient;

pub async fn get_single(
    Path(id): Path<u32>,
    State(hyper): State<ReqwestClient>,
) -> Result<Json<models::Show>, Error> {
    let anime = kitsu::anime::single(hyper, id).await?;
    let show = anime.data.try_into()?;
    Ok(Json(show))
}

pub async fn get_collection(
    State(hyper): State<ReqwestClient>,
) -> Result<Json<Vec<models::Show>>, Error> {
    let anime = kitsu::anime::collection(hyper).await?;
    let show: Result<Vec<_>, _> = anime.data.into_iter().map(|d| d.try_into()).collect();
    Ok(Json(show?))
}
