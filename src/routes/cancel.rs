use crate::server::state::ApiState;
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
    /// Task id that was scheduled for cancellation
    #[schema(example = "0")]
    id: String,
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
        (status = 202, description = "Task was scheduled for cancellation", body = CancelReponse, example = json!(CancelReponse{id: String::from("some-id")})),
        (status = 404, description = "Task not found for this chat id"),
        (status = 400, description = "Chat id missing"),
        (status = 400, description = "Api key missing"),
        (status = 401, description = "Api key invalid"),
    ),
    security(
        ("api_key" = []),
        ("chat_id" = [])
    ),
)]
pub async fn cancel(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<CancelReponse, StatusCode> {
    let _ = state.cancel_task(&id).await.ok_or(StatusCode::NOT_FOUND)?;

    Ok(CancelReponse { id })
}
