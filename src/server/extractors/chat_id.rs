use crate::server::response::ApiError;
use axum::{extract::FromRequestParts, http::request::Parts};
use serde::{Deserialize, Serialize};

use super::query::Query;

#[derive(Serialize, Deserialize)]
struct ChatIdContainer {
    /// Chat id. generated using the `/api/request_chat_id` endpoint
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
