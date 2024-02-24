use crate::server::{
    extractors::chat_id::ChatId,
    response::ApiError,
    state::ApiState,
    task::{ProcessStatus, Status},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct StatusReponse {
    /// Status of a given task
    status: Status,
}

impl IntoResponse for StatusReponse {
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

/// Get the status of a task
#[utoipa::path(
    get,
    path = "/api/status/{id}", 
    params(
        ("id" = String, Path, description = "Task id. generated using the `/api/run` endpoint"),
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint")
    ),
    tag = "task",
    responses(
        (status = 200, description = "Status of a given task", body = StatusReponse, example = json!(StatusReponse{status: Status::Process(ProcessStatus::Running)})),
        (status = 404, description = "Task not found for this chat id"),
        (status = 400, description = "Chat id missing. Api key missing."),
        (status = 401, description = "Api key invalid"),
    ),
    security(
        ("api_key" = []),
    ),
)]
pub async fn status(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    ChatId(chat_id): ChatId,
) -> Result<StatusReponse, ApiError> {
    let status = state
        .task_status(&id, &chat_id)
        .await
        .ok_or(ApiError::NotFound)?;

    Ok(StatusReponse { status })
}
