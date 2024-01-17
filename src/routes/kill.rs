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
pub struct KillReponse {
    status: Status,
}

impl IntoResponse for KillReponse {
    fn into_response(self) -> Response {
        (StatusCode::ACCEPTED, Json(self)).into_response()
    }
}

#[utoipa::path(
    put,
    path = "/api/kill/{id}", 
    params(
        ("id" = String, Path, description = "Task id")
    ),
    tag = "task",
    responses(
        (status = 202, description = "Task was killed", body = KillReponse, example = json!(KillReponse{status: Status::Killed})),
        (status = 400, description = "Task not found"),
    )
)]
pub async fn kill(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<KillReponse, StatusCode> {
    let status = state.kill_task(&id).await.ok_or(StatusCode::NOT_FOUND)?;

    Ok(KillReponse { status })
}
