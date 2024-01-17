use crate::server::{state::ApiState, task::Status};
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
    status: Status,
}

impl IntoResponse for StatusReponse {
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

#[utoipa::path(
    get,
    path = "/api/status/{id}", 
    params(
        ("id" = String, Path, description = "Task id")
    ),
    tag = "task",
    responses(
        (status = 200, description = "Status of a given task", body = StatusReponse, example = json!(StatusReponse{status: Status::Running})),
        (status = 400, description = "Task not found"),
    )
)]
pub async fn status(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<StatusReponse, StatusCode> {
    let status = state.task_status(&id).await.ok_or(StatusCode::NOT_FOUND)?;

    Ok(StatusReponse { status })
}
