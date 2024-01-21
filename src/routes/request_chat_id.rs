use crate::server::state::ApiState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct RequestChatIdReponse {
    #[schema(example = "0")]
    id: String,
}

impl IntoResponse for RequestChatIdReponse {
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

#[utoipa::path(
    get,
    path = "/api/request_chat_id",
    tag = "task",
    responses(
        (status = 200, description = "Generated chat id for this session", body = RequestChatIdReponse, example = json!(RequestChatIdReponse{id: String::from("some-id")})),
        (status = 400, description = "Api key missing"),
        (status = 401, description = "Api key invalid"),
    ),
    security(
        ("api_key" = []),
    ),
)]
pub async fn request_chat_id(State(state): State<ApiState>) -> RequestChatIdReponse {
    let id = state.generate_random_chat_id();

    RequestChatIdReponse { id }
}
