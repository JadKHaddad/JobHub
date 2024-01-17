use crate::server::state::ServerState;
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
    path = "/run", 
    tag = "task",
    responses(
        (status = 201, description = "Task was creating and is running", body = RunReponse),
    )
)]
pub async fn run(State(state): State<ServerState>) -> RunReponse {
    let id = state.run_task().await;

    RunReponse { id }
}
