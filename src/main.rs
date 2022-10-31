mod models;

#[macro_use]
extern crate rocket;

use std::num::ParseIntError;
use hyper_tls::HttpsConnector;
use rocket::{Request, Response};
use rocket::http::{ContentType, Status};
use rocket::response::Responder;
use rocket::serde::{json::Json};
use tracing::error;
use thiserror::Error as ThisError;
use kitsu;
use nyaa;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error(transparent)]
    RocketError(#[from] rocket::Error),
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
    #[error(transparent)]
    Kitsu(#[from] kitsu::Error),
    #[error(transparent)]
    Nyaa(#[from] nyaa::Error),
    #[error(transparent)]
    ParseIntError(#[from] ParseIntError),
}

#[rocket::async_trait]
impl<'r, 'o: 'r> Responder<'r, 'o> for Error {
    fn respond_to(self, _: &'r Request<'_>) -> rocket::response::Result<'o> {
        let mut builder = Response::build();
        builder.header(ContentType::Plain);
        match self {
            Self::ParseIntError(_) => builder.status(Status::BadRequest),
            _ => builder.status(Status::InternalServerError),
        };
        builder.ok()
    }
}


#[rocket::main]
async fn main() -> Result<(), Error> {
    // initialize tracing
    tracing_subscriber::fmt::init();
    // our router
    let rocket = rocket::build()
        .mount("/anime", routes![get_anime])
        .mount("/downloads", routes![get_downloads]);

    let _ignite = rocket.launch().await?;
    Ok(())
}

#[get("/<id>")]
async fn get_anime(id: u32) -> Result<Json<models::Show>, Error> {
    let client: kitsu::Client = Default::default();
    let anime = client.get_anime(id).await?;
    Ok(Json(anime.data.try_into()?))
}

#[get("/")]
async fn get_downloads() -> Result<Json<Vec<models::Episode>>, Error> {
    let client: nyaa::Client = Default::default();
    let episodes = client.get_anime().await;
    let result = episodes?.iter().map(|e| e.into()).collect();
    Ok(Json(result))
}

