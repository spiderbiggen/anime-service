use rocket::serde::json::Json;
use std::result::Result;
use crate::{models, Error, HYPER};

#[get("/<id>")]
pub async fn get_anime(id: u32) -> Result<Json<models::Show>, Error> {
    let anime = kitsu::anime::single(HYPER.clone(), id).await?;
    let show = anime.data.try_into()?;
    Ok(Json(show))
}
