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
pub struct CancelReponse {
    status: Status,
}

impl IntoResponse for CancelReponse {
    fn into_response(self) -> Response {
        (StatusCode::ACCEPTED, Json(self)).into_response()
    }
}

#[utoipa::path(
    put,
    path = "/api/cancel/{id}", 
    params(
        ("id" = String, Path, description = "Task id")
    ),
    tag = "task",
    responses(
        (status = 202, description = "Task was canceled", body = CancelReponse, example = json!(CancelReponse{status: Status::Canceled})),
        (status = 400, description = "Task not found"),
    )
)]
pub async fn cancel(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<CancelReponse, StatusCode> {
    let status = state.cancel_task(&id).await.ok_or(StatusCode::NOT_FOUND)?;

    Ok(CancelReponse { status })
}
