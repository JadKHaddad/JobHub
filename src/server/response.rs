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
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized. See server logs"),
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "error")]
pub enum ApiError {
    Unauthorized,
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
