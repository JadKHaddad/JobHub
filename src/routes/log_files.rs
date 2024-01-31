//! Routes and responses for downloading log files
use crate::server::{
    extractors::{chat_id::ChatId, query::Query},
    state::ApiState,
};

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use utoipa::{IntoParams, ToSchema};

#[derive(Serialize, ToSchema)]
pub struct ListLogfilesResponse {
    /// List of names of available log files
    files: Vec<String>,
}

#[derive(Deserialize, ToSchema, IntoParams)]
pub struct GetLogFileQuery {
    /// Name of the log file to download
    file_name: String,
}

impl IntoResponse for ListLogfilesResponse {
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

#[utoipa::path(
    get,
    path = "/api/list_log_files", 
    tag = "files",
    params(
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint")
    ),
    responses(
        (status = 201, description = "Task was scheduled for running", body = RunReponse, example = json!(ListLogfilesResponse{files: vec![String::from("file_1.log"), String::from("file_2.log")]})),
        (status = 400, description = "Chat id missing"),
        (status = 400, description = "Api key missing"),
        (status = 401, description = "Api key invalid"),
    ),
    security(
        ("api_key" = []),
    ),
)]
pub async fn list_log_files(
    State(_state): State<ApiState>,
    ChatId(_chat_id): ChatId,
) -> ListLogfilesResponse {
    todo!()
}

#[utoipa::path(
    get,
    path = "/api/get_log_file", 
    params(
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint"),
        ("file_name" = String, Query, description = "Name of the log file to download")
    ),
    tag = "files",
    responses(
        (status = 400, description = "Chat id missing"),
        (status = 400, description = "Api key missing"),
        (status = 401, description = "Api key invalid"),
        (status = 400, description = "Query invalid"),
    ),
    security(
        ("api_key" = []),
    ),
)]
pub async fn get_log_file(
    State(state): State<ApiState>,
    ChatId(chat_id): ChatId,
    Query(file_name): Query<GetLogFileQuery>,
) -> String {
    include_str!("sys.log").to_owned()
}
