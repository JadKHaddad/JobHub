use crate::server::response::ApiError;
use axum::{
    extract::{FromRequestParts, Query},
    http::request::Parts,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct ChatIdContainer {
    chat_id: String,
}

pub struct ChatId(pub String);

#[axum::async_trait]
impl<S> FromRequestParts<S> for ChatId
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = Query::<ChatIdContainer>::from_request_parts(parts, _state)
            .await
            .map_err(|_| ApiError::ChatIdMissing)?;

        Ok(Self(query.0.chat_id))
    }
}
