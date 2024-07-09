use crate::server::{
    extractors::chat_id::ChatId,
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
pub struct StatusOkResponse {
    /// Status of a given task
    status: Status,
}

#[derive(Serialize, ToSchema)]
pub enum StatusErrorResponse {
    NotFound,
}

impl IntoResponse for StatusOkResponse {
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

impl IntoResponse for StatusErrorResponse {
    fn into_response(self) -> Response {
        (StatusCode::NOT_FOUND, Json(self)).into_response()
    }
}

/// Get the status of a task
#[utoipa::path(
    get,
    path = "/api/status/{id}", 
    params(
        ("id" = String, Path, description = "Task id. generated using the `/api/download_zip_file` endpoint."),
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint.")
    ),
    tag = "task",
    responses(
        (status = 200, description = "Status of a given task", body = StatusOkReponse, example = json!(StatusOkReponse{status: Status::Process(ProcessStatus::Running)})),
        (status = 404, description = "Task not found for this chat id", body = StatusErrorReponse, example = json!(StatusErrorReponse::NotFound)),
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
) -> Result<StatusOkResponse, StatusErrorResponse> {
    let status = state
        .task_status(&id, &chat_id)
        .await
        .ok_or(StatusErrorResponse::NotFound)?;

    Ok(StatusOkResponse { status })
}
