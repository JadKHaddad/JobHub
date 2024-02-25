use crate::server::{extractors::chat_id::ChatId, state::ApiState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct CancelOkReponse {
    /// Task id that was scheduled for cancellation
    #[schema(example = "0")]
    id: String,
}

#[derive(Serialize, ToSchema)]
pub enum CancelErrorReponse {
    NotFound,
}

impl IntoResponse for CancelOkReponse {
    fn into_response(self) -> Response {
        (StatusCode::ACCEPTED, Json(self)).into_response()
    }
}

impl IntoResponse for CancelErrorReponse {
    fn into_response(self) -> Response {
        (StatusCode::NOT_FOUND, Json(self)).into_response()
    }
}

/// Schedule a task for cancellation
#[utoipa::path(
    put,
    path = "/api/cancel/{id}", 
    params(
        ("id" = String, Path, description = "Task id. generated using the `/api/run` endpoint"),
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint")
    ),
    tag = "task",
    responses(
        (status = 202, description = "Task was scheduled for cancellation", body = CancelOkReponse, example = json!(CancelOkReponse{id: String::from("some-id")})),
        (status = 404, description = "Task not found for this chat id", body = CancelErrorReponse, example = json!(CancelErrorReponse::NotFound)),
        (status = 400, description = "Chat id missing. Api key missing"),
        (status = 401, description = "Api key invalid"),
    ),
    security(
        ("api_key" = []),
    ),
)]
pub async fn cancel(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    ChatId(chat_id): ChatId,
) -> Result<CancelOkReponse, CancelErrorReponse> {
    let _ = state
        .cancel_task(&id, &chat_id)
        .await
        .ok_or(CancelErrorReponse::NotFound)?;

    Ok(CancelOkReponse { id })
}
