#[cfg(feature = "reqwest")]
use hyper_tls::HttpsConnector;

pub mod api;

#[cfg(feature = "hyper")]
pub use crate::hyper::*;

#[cfg(feature = "reqwest")]
pub struct Client(hyper::client::Client);

#[cfg(feature = "hyper")]
mod hyper {
    use crate::api::ApiError;
    use async_trait::async_trait;

    use bytes::Bytes;
    use http::request::Builder as RequestBuilder;
    use http::Response;

    use hyper::body::to_bytes;
    use hyper::client::{Client as HyperClient, HttpConnector};
    use hyper::Body;
    #[cfg(feature = "hyper-tls")]
    use hyper_tls::HttpsConnector;
    use log::debug;
    use url::Url;

    #[cfg(not(feature = "hyper-tls"))]
    pub type Connector = HyperClient<HttpConnector>;
    #[cfg(feature = "hyper-tls")]
    pub type Connector = HyperClient<HttpsConnector<HttpConnector>>;

    pub struct Client {
        base_url: Url,
        hyper_client: Connector,
    }

    #[async_trait]
    impl crate::api::Client for Client {
        fn rest_endpoint(&self, endpoint: &str) -> Result<Url, ApiError> {
            debug!("REST api call {}", endpoint);
            Ok(self.base_url.join(endpoint)?)
        }

        async fn rest(
            &self,
            request: RequestBuilder,
            body: Vec<u8>,
        ) -> Result<Response<Bytes>, ApiError> {
            let c = self.hyper_client.clone();
            let req = request.body(Body::from(body))?;
            let (parts, body) = c.request(req).await?.into_parts();
            let body = to_bytes(body).await?;
            Ok(Response::from_parts(parts, body))
        }
    }
}
