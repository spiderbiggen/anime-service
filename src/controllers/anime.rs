use std::result::Result;
use axum::{Json, extract::Path};

use crate::{models, Error, HYPER};

pub async fn get_single(Path(id): Path<u32>) -> Result<Json<models::Show>, Error> {
    let anime = kitsu::anime::single(HYPER.clone(), id).await?;
    let show = anime.data.try_into()?;
    Ok(Json(show))
}
