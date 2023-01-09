use super::{client::Client, error::ApiError};
use async_trait::async_trait;

#[async_trait]
pub trait Query<T, C>
where
    C: Client,
{
    async fn query(&self, client: &C) -> Result<T, ApiError>;
}
