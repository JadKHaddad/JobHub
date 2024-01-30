use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct ApiErrorResponse {
    #[serde(skip)]
    status_code: StatusCode,
    err: ApiError,
    msg: &'static str,
}

impl From<ApiError> for ApiErrorResponse {
    fn from(value: ApiError) -> Self {
        let (status_code, msg) = match &value {
            ApiError::ChatIdMissing => (StatusCode::BAD_REQUEST, "Chat id missing"),
            ApiError::ApiKeyMissing => (StatusCode::BAD_REQUEST, "Api key missing"),
            ApiError::ApiKeyInvalid => (StatusCode::UNAUTHORIZED, "Api key invalid"),
            ApiError::QueryInvalid => (StatusCode::BAD_REQUEST, "Query invalid"),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "Not found"),
            ApiError::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error. See server logs",
            ),
        };

        Self {
            status_code,
            err: value,
            msg,
        }
    }
}

impl IntoResponse for ApiErrorResponse {
    fn into_response(self) -> Response {
        (self.status_code, Json(self)).into_response()
    }
}

/// Invalid chat id is not exposed by the api.
/// Every task has a chat id associated with it.
/// If the client tries to access a task with an invalid chat id,
/// the server will return a 404, preventing the client from guessing valid task ids.
/// In other words, 404 means task not found for this chat id.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "error")]
pub enum ApiError {
    ChatIdMissing,
    ApiKeyMissing,
    ApiKeyInvalid,
    QueryInvalid,
    NotFound,
    InternalServerError,
}

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        let err: anyhow::Error = err.into();
        let err = format!("{err:#}");
        tracing::error!(%err, "Internal server error");

        ApiError::InternalServerError
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let err = ApiErrorResponse::from(self);

        err.into_response()
    }
}
