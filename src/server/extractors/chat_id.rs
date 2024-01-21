use crate::server::response::ApiError;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap},
};

pub struct ChatId(pub String);

#[axum::async_trait]
impl<S> FromRequestParts<S> for ChatId
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let headers = HeaderMap::from_request_parts(parts, _state)
            .await
            .map_err(|_| ApiError::ChatIdMissing)?;

        let chat_id = headers
            .get("chat_id")
            .ok_or(ApiError::ChatIdMissing)?
            .to_str()
            .map_err(|_| ApiError::ChatIdMissing)?;

        Ok(Self(chat_id.to_string()))
    }
}
