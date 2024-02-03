//! Routes and responses for downloading log files
use crate::server::{
    extractors::{chat_id::ChatId, query::Query},
    state::ApiState,
};

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
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

/// List available log files
#[utoipa::path(
    get,
    path = "/api/list_log_files", 
    tag = "files",
    params(
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint")
    ),
    responses(
        (status = 200, description = "List of names of available log files", body = ListLogfilesResponse, example = json!(ListLogfilesResponse{files: vec![String::from("file_1.log"), String::from("file_2.log")]})),
        (status = 400, description = "Chat id missing. Api key missing"),
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
    ListLogfilesResponse {
        files: vec![String::from("file_1.log"), String::from("file_2.log")],
    }
}

/// Download a log file as text/plain
#[utoipa::path(
    get,
    path = "/api/get_log_file_text", 
    params(
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint"),
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
    ChatId(chat_id): ChatId,
    Query(file_name): Query<GetLogFileQuery>,
) -> String {
    include_str!("sys.log").to_owned()
}

#[derive(Deserialize, ToSchema, IntoParams)]
pub struct GetLogFileLimitedQuery {
    /// Name of the log file to download
    file_name: String,
    /// Maximum number of bytes to download
    limit: u64,
}

/// Download a log file as text/plain with limited number of bytes
#[utoipa::path(
    get,
    path = "/api/get_log_file_text_limited", 
    params(
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint"),
        ("file_name" = String, Query, description = "Name of the log file to download"),
        ("limit" = u64, Query, description = "Maximum number of bytes to download")
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
pub async fn get_log_file_text_limited(
    State(state): State<ApiState>,
    ChatId(chat_id): ChatId,
    Query(query): Query<GetLogFileLimitedQuery>,
) -> String {
    let str = include_str!("sys.log");
    if query.limit as usize > str.len() {
        str.to_owned()
    } else {
        str[..query.limit as usize].to_owned()
    }
}

/// Download a log file as application/octet-stream
#[utoipa::path(
    get,
    path = "/api/get_log_file_octet", 
    params(
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint"),
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
pub async fn get_log_file_octet(
    State(state): State<ApiState>,
    ChatId(chat_id): ChatId,
    Query(file_name): Query<GetLogFileQuery>,
) -> Vec<u8> {
    include_bytes!("sys.log").to_vec()
}

/// Download a log file as application/octet-stream with limited number of bytes
#[utoipa::path(
    get,
    path = "/api/get_log_file_octet_limited", 
    params(
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint"),
        ("file_name" = String, Query, description = "Name of the log file to download"),
        ("limit" = u64, Query, description = "Maximum number of bytes to download")
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
pub async fn get_log_file_octet_limited(
    State(state): State<ApiState>,
    ChatId(chat_id): ChatId,
    Query(query): Query<GetLogFileLimitedQuery>,
) -> Vec<u8> {
    let bytes = include_bytes!("sys.log");

    if query.limit as usize > bytes.len() {
        bytes.to_vec()
    } else {
        bytes[..query.limit as usize].to_vec()
    }
}

#[derive(Deserialize, ToSchema, IntoParams)]
pub struct GetLogFileOffsetLimitedQuery {
    /// Name of the log file to download
    file_name: String,
    /// Offset in bytes
    offset: u64,
    /// Maximum number of bytes to download
    limit: u64,
}

/// Download a log file as application/octet-stream with offset and limited number of bytes
#[utoipa::path(
    get,
    path = "/api/get_log_file_octet_offset_limited", 
    params(
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint"),
        ("file_name" = String, Query, description = "Name of the log file to download"),
        ("offset" = u64, Query, description = "Offset in bytes"),
        ("limit" = u64, Query, description = "Maximum number of bytes to download")
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
pub async fn get_log_file_octet_offset_limited(
    State(state): State<ApiState>,
    ChatId(chat_id): ChatId,
    Query(query): Query<GetLogFileOffsetLimitedQuery>,
) -> Vec<u8> {
    let bytes = include_bytes!("sys.log");
    if query.offset as usize > bytes.len() {
        return vec![];
    }

    if query.limit as usize > bytes.len() - query.offset as usize {
        bytes[query.offset as usize..].to_vec()
    } else {
        bytes[query.offset as usize..query.offset as usize + query.limit as usize].to_vec()
    }
}

/// Download a log file as application/octet-stream with attachment with offset and limited number of bytes
#[utoipa::path(
    get,
    path = "/api/get_log_file_octet_offset_limited_attachment", 
    params(
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint"),
        ("file_name" = String, Query, description = "Name of the log file to download"),
        ("offset" = u64, Query, description = "Offset in bytes"),
        ("limit" = u64, Query, description = "Maximum number of bytes to download")
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
pub async fn get_log_file_octet_offset_limited_attachment(
    State(state): State<ApiState>,
    ChatId(chat_id): ChatId,
    Query(query): Query<GetLogFileOffsetLimitedQuery>,
) -> (HeaderMap, Vec<u8>) {
    // use streams: https://github.com/tokio-rs/axum/discussions/608
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "application/octet-stream".parse().unwrap());
    headers.insert(
        "Content-Disposition",
        "attachment; filename=sys.log".parse().unwrap(),
    );
    let bytes = include_bytes!("sys.log");
    if query.offset as usize > bytes.len() {
        return (headers, vec![]);
    }

    if query.limit as usize > bytes.len() - query.offset as usize {
        (headers, bytes[query.offset as usize..].to_vec())
    } else {
        (
            headers,
            bytes[query.offset as usize..query.offset as usize + query.limit as usize].to_vec(),
        )
    }
}
