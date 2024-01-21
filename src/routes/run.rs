use crate::server::{extractors::chat_id::ChatId, state::ApiState};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct RunReponse {
    /// Task id that was scheduled for running
    #[schema(example = "0")]
    id: String,
}

impl IntoResponse for RunReponse {
    fn into_response(self) -> Response {
        (StatusCode::CREATED, Json(self)).into_response()
    }
}

#[utoipa::path(
    post,
    path = "/api/run", 
    tag = "task",
    responses(
        (status = 201, description = "Task was scheduled for running", body = RunReponse, example = json!(RunReponse{id: String::from("some-id")})),
        (status = 400, description = "Chat id missing"),
        (status = 400, description = "Api key missing"),
        (status = 401, description = "Api key invalid"),
    ),
    security(
        ("api_key" = []),
        ("chat_id" = [])
    ),
)]
pub async fn run(State(state): State<ApiState>, ChatId(chat_id): ChatId) -> RunReponse {
    let id = state.run_task(chat_id).await;

    RunReponse { id }
}
