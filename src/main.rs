mod models;
mod controllers;

#[macro_use]
extern crate rocket;

use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use once_cell::sync::Lazy;
use rocket::http::{ContentType, Status};
use rocket::response::Responder;
use rocket::{Request, Response};
use std::num::ParseIntError;
use thiserror::Error as ThisError;
use tracing::error;
use crate::controllers::{anime, downloads};

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

static HYPER: Lazy<hyper::Client<HttpsConnector<HttpConnector>>> = Lazy::new(||hyper::Client::builder().build(HttpsConnector::new()));

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
        .mount("/anime", routes![anime::get_anime])
        .mount("/downloads", routes![downloads::get, downloads::get_groups]);

    let _ignite = rocket.launch().await?;
    Ok(())
}