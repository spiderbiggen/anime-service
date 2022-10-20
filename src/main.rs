mod models;

#[macro_use]
extern crate rocket;

use std::num::ParseIntError;
use rocket::{Request, Response};
use rocket::http::{ContentType, Status};
use rocket::response::Responder;
use rocket::serde::{json::Json};
use tracing::error;
use kitsu::Client;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error(transparent)]
    RocketError(#[from] rocket::Error),
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
    #[error(transparent)]
    Kitsu(#[from] kitsu::Error),
    #[error(transparent)]
    ParseIntError(#[from] ParseIntError),
}

#[rocket::async_trait]
impl<'r, 'o: 'r> Responder<'r, 'o> for Error {
    fn respond_to(self, _: &'r Request<'_>) -> rocket::response::Result<'o> {
        let mut builder = Response::build();
        builder.header(ContentType::Plain);
        match self {
            Self::ParseIntError(_) => {
                builder.status(Status::BadRequest)
            }
            _ => {
                builder.status(Status::InternalServerError)
            }
        };
        builder.ok()
    }
}


#[tokio::main]
async fn main() -> Result<(), Error> {
    // initialize tracing
    tracing_subscriber::fmt::init();

    // our router
    let rocket = rocket::build()
        .mount("/", routes![get_anime]);

    let _ignite = rocket.launch().await?;
    Ok(())
}

#[get("/anime/<id>")]
async fn get_anime(id: u32) -> Result<Json<models::Show>, Error> {
    let client: Client = Default::default();
    let anime = client.get_anime(id).await?;
    Ok(Json(anime.data.try_into()?))
}

