use crate::server::{
    extractors::{chat_id::ChatId, query::Query},
    state::{ApiState, GsLogToLocustConverterError},
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
pub struct GsLogToLocustConverterOkResponse {
    /// Task id that was scheduled for running
    #[schema(example = "0")]
    id: String,
}

#[derive(Serialize, ToSchema)]
pub enum GsLogToLocustConverterErrorResponse {
    NotFound,
}

impl From<GsLogToLocustConverterError> for GsLogToLocustConverterErrorResponse {
    fn from(err: GsLogToLocustConverterError) -> Self {
        match err {
            GsLogToLocustConverterError::NotFound => GsLogToLocustConverterErrorResponse::NotFound,
        }
    }
}

impl IntoResponse for GsLogToLocustConverterOkResponse {
    fn into_response(self) -> Response {
        (StatusCode::CREATED, Json(self)).into_response()
    }
}

impl IntoResponse for GsLogToLocustConverterErrorResponse {
    fn into_response(self) -> Response {
        match self {
            GsLogToLocustConverterErrorResponse::NotFound => {
                (StatusCode::NOT_FOUND, Json(self)).into_response()
            }
        }
    }
}

#[derive(Deserialize)]
pub struct GsLogToLocustConverterQuery {
    /// Name of the project
    project_name: String,
}

/// Converts the format of log files given in the GS log format to the format used by locust (Locust log format). 
#[utoipa::path(
    post,
    path = "/api/gs_log_to_locust_converter", 
    params(
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint"),
        ("project_name" = String, Query, description = "Name of the project.")
    ),
    tag = "convert",
    responses(
        (status = 201, description = "Task was scheduled for running", body = GsLogToLocustConverterOkResponse, example = json!(GsLogToLocustConverterOkResponse{id: String::from("some-id")})),
        (status = 400, description = "Chat id missing, Api key missing"),
        (status = 401, description = "Api key invalid"),
    ),
    security(
        ("api_key" = []),
    ),
)]
pub async fn gs_log_to_locust_converter(
    State(state): State<ApiState>,
    ChatId(chat_id): ChatId,
    Query(query): Query<GsLogToLocustConverterQuery>,
) -> Result<GsLogToLocustConverterOkResponse, GsLogToLocustConverterErrorResponse> {
    let project_name = query.project_name;

    let id = state
        .run_gs_log_to_locust_converter_task(chat_id, project_name)
        .await?;

    Ok(GsLogToLocustConverterOkResponse { id })
}
