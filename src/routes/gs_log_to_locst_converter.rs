use crate::server::{
    extractors::{chat_id::ChatId, query::Query},
    state::{ApiState, GsLogToLocstConverterError},
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct GsLogToLocstConverterOkResponse {
    /// Task id that was scheduled for running
    #[schema(example = "0")]
    id: String,
}

#[derive(Serialize, ToSchema)]
pub enum GsLogToLocstConverterErrorResponse {
    NotFound,
}

impl From<GsLogToLocstConverterError> for GsLogToLocstConverterErrorResponse {
    fn from(err: GsLogToLocstConverterError) -> Self {
        match err {
            GsLogToLocstConverterError::NotFound => GsLogToLocstConverterErrorResponse::NotFound,
        }
    }
}

impl IntoResponse for GsLogToLocstConverterOkResponse {
    fn into_response(self) -> Response {
        (StatusCode::CREATED, Json(self)).into_response()
    }
}

impl IntoResponse for GsLogToLocstConverterErrorResponse {
    fn into_response(self) -> Response {
        match self {
            GsLogToLocstConverterErrorResponse::NotFound => {
                (StatusCode::NOT_FOUND, Json(self)).into_response()
            }
        }
    }
}

#[derive(Deserialize)]
pub struct GsLogToLocstConverterQuery {
    /// Name of the project
    project_name: String,
}

/// TODO: Add description
#[utoipa::path(
    post,
    path = "/api/gs_log_to_locst_converter", 
    params(
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint"),
        ("project_name" = String, Query, description = "Name of the project.")
    ),
    tag = "convert",
    responses(
        (status = 201, description = "Task was scheduled for running", body = GsLogToLocstConverterOkResponse, example = json!(GsLogToLocstConverterOkResponse{id: String::from("some-id")})),
        (status = 400, description = "Chat id missing, Api key missing"),
        (status = 401, description = "Api key invalid"),
    ),
    security(
        ("api_key" = []),
    ),
)]
pub async fn gs_log_to_locst_converter(
    State(state): State<ApiState>,
    ChatId(chat_id): ChatId,
    Query(query): Query<GsLogToLocstConverterQuery>,
) -> Result<GsLogToLocstConverterOkResponse, GsLogToLocstConverterErrorResponse> {
    let project_name = query.project_name;

    let id = state
        .run_gs_log_to_locst_converter_task(chat_id, project_name)
        .await?;

    Ok(GsLogToLocstConverterOkResponse { id })
}
