//! Routes and responses for downloading log files
use crate::server::{
    extractors::{chat_id::ChatId, query::Query},
    state::{ApiState, GetFileError, ListFilesError},
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
pub struct ListLogfilesOkResponse {
    /// List of names of available log files
    files: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub enum ListLogfilesErrorResponse {
    NotFound,
    ServerError,
}

impl IntoResponse for ListLogfilesErrorResponse {
    fn into_response(self) -> Response {
        match self {
            ListLogfilesErrorResponse::NotFound => {
                (StatusCode::NOT_FOUND, Json(self)).into_response()
            }
            ListLogfilesErrorResponse::ServerError => {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(self)).into_response()
            }
        }
    }
}

impl From<ListFilesError> for ListLogfilesErrorResponse {
    fn from(err: ListFilesError) -> Self {
        match err {
            ListFilesError::NotFound => ListLogfilesErrorResponse::NotFound,
            ListFilesError::IoError(_) => ListLogfilesErrorResponse::ServerError,
        }
    }
}

impl IntoResponse for ListLogfilesOkResponse {
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

#[derive(Deserialize)]
pub struct ListFilesQuery {
    /// Name of the project
    project_name: String,
}

/// List available log files
#[utoipa::path(
    get,
    path = "/api/list_log_files", 
    tag = "files",
    params(
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint"),
        ("project_name" = String, Query, description = "Name of the project"),
    ),
    responses(
        (status = 200, description = "List of names of available log files", body = ListLogfilesOkResponse, example = json!(ListLogfilesOkResponse{files: vec![String::from("file_1.log"), String::from("file_2.log")]})),
        (status = 400, description = "Chat id missing. Api key missing"),
        (status = 401, description = "Api key invalid"),
    ),
    security(
        ("api_key" = []),
    ),
)]
pub async fn list_log_files(
    State(state): State<ApiState>,
    ChatId(_chat_id): ChatId,
    Query(query): Query<ListFilesQuery>,
) -> Result<ListLogfilesOkResponse, ListLogfilesErrorResponse> {
    let files = state.list_files(query.project_name).await?;

    Ok(ListLogfilesOkResponse { files })
}

#[derive(Serialize, ToSchema)]
pub enum GetLogFileErrorResponse {
    NotFound,
    ServerError,
}

impl IntoResponse for GetLogFileErrorResponse {
    fn into_response(self) -> Response {
        match self {
            GetLogFileErrorResponse::NotFound => {
                (StatusCode::NOT_FOUND, Json(self)).into_response()
            }
            GetLogFileErrorResponse::ServerError => {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(self)).into_response()
            }
        }
    }
}

impl From<GetFileError> for GetLogFileErrorResponse {
    fn from(err: GetFileError) -> Self {
        match err {
            GetFileError::NotFound => GetLogFileErrorResponse::NotFound,
            GetFileError::IoError(_) => GetLogFileErrorResponse::ServerError,
        }
    }
}

#[derive(Deserialize)]
pub struct GetLogFileQuery {
    /// Name of the project
    project_name: String,
    /// Name of the log file to download
    file_name: String,
}

/// Download a log file as text/plain
#[utoipa::path(
    get,
    path = "/api/get_log_file_text", 
    params(
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint"),
        ("project_name" = String, Query, description = "Name of the project"),
        ("file_name" = String, Query, description = "Name of the log file to download")
    ),
    tag = "files",
    responses(
        (status = 200, description = "Log file", body = String),
        (status = 400, description = "Chat id missing. Api key missing. Query invalid"),
        (status = 401, description = "Api key invalid"),
    ),
    security(
        ("api_key" = []),
    ),
)]
pub async fn get_log_file_text(
    State(state): State<ApiState>,
    ChatId(_chat_id): ChatId,
    Query(query): Query<GetLogFileQuery>,
) -> Result<String, GetLogFileErrorResponse> {
    let file = state.get_file(query.project_name, query.file_name).await?;

    Ok(file)
}
