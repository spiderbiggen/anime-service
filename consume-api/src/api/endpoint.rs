use std::borrow::Cow;

use async_trait::async_trait;
use http::{header, Method, Request, Uri};
use serde::de::DeserializeOwned;
use url::Url;

use super::error::{ApiError, BodyError};
use super::query::Query;
use super::{client::Client, QueryParams};

fn url_to_http_uri(url: Url) -> Result<Uri, ApiError> {
    Ok(url.as_str().parse::<Uri>()?)
}
pub trait Endpoint {
    fn method(&self) -> Method;
    fn endpoint(&self) -> Cow<'static, str>;
    fn parameters(&self) -> QueryParams {
        QueryParams::default() // Many endpoints don't have parameters
    }
    fn body(&self) -> Result<Option<(&'static str, Vec<u8>)>, BodyError> {
        Ok(None) // Many endpoints also do not have request bodies
    }
}

#[async_trait]
impl<E, T, C> Query<T, C> for E
where
    E: Endpoint + Sync,
    T: DeserializeOwned + 'static,
    C: Client + Sync,
{
    async fn query(&self, client: &C) -> Result<T, ApiError> {
        let mut url = client.rest_endpoint(&self.endpoint())?;
        self.parameters().add_to_url(&mut url);

        let req = Request::builder()
            .method(self.method())
            .uri(url_to_http_uri(url)?);
        let (req, data) = match self.body()? {
            Some((mime, data)) => (req.header(header::CONTENT_TYPE, mime), data),
            None => (req, Vec::new()),
        };
        let rsp = client.rest(req, data).await?;
        let status = rsp.status();
        let Ok(v) = serde_json::from_slice(rsp.body()) else {
            return Err(ApiError::server_error(status, rsp.body()));
        };
        if !status.is_success() {
            return Err(ApiError::server_error(status, rsp.body()));
        }

        serde_json::from_value::<T>(v).map_err(ApiError::data_type::<T>)
    }
}
