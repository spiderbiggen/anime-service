use async_trait::async_trait;
use bytes::Bytes;
use http::{request::Builder as RequestBuilder, Response};
use url::Url;

use super::error::ApiError;

#[async_trait]
pub trait Client {
    fn rest_endpoint(&self, endpoint: &str) -> Result<Url, ApiError>;
    async fn rest(
        &self,
        request: RequestBuilder,
        body: Vec<u8>,
    ) -> Result<Response<Bytes>, ApiError>;
}
