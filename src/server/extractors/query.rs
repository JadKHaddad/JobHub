use crate::server::response::ApiError;
use axum::{
    extract::{FromRequestParts, Query as AxumQuery},
    http::request::Parts,
};
use serde::de::DeserializeOwned;

/// A Wrapper around [`axum::extract::Query`] that rejects with an [`ApiError`]
pub struct Query<T>(pub T);

#[axum::async_trait]
impl<T, S> FromRequestParts<S> for Query<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = AxumQuery::<T>::from_request_parts(parts, _state)
            .await
            .map_err(|_| ApiError::QueryInvalid)?;

        Ok(Self(query.0))
    }
}
