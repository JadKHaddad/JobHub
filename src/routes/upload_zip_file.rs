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
use utoipa::{IntoParams, ToSchema};

#[derive(Serialize, ToSchema)]
pub struct DownloadZipFileOkReponse {
    /// Task id that was scheduled for running
    #[schema(example = "0")]
    id: String,
}

#[derive(Serialize, ToSchema)]
pub struct DownloadZipFileErrorReponse {
    /// Error message
    #[schema(example = "Invalid scheme")]
    message: String,
}

impl IntoResponse for DownloadZipFileOkReponse {
    fn into_response(self) -> Response {
        (StatusCode::CREATED, Json(self)).into_response()
    }
}

impl IntoResponse for DownloadZipFileErrorReponse {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(self)).into_response()
    }
}

#[derive(Deserialize, ToSchema, IntoParams)]
pub struct DownloadZipFileQuery {
    /// Name of the project
    project_name: String,
    /// Google drive share link for the zip file
    #[schema(
        example = "https://drive.google.com/file/d/1FAjgIAL81UvshCn2owqlcPnvXl_k0cP2/view?usp=sharing"
    )]
    google_drive_share_link: String,
}

/// Schedule a download of a zip file
///
/// This endpoint will schedule a task for running. The task will be executed asynchronously.
#[utoipa::path(
    post,
    path = "/api/download_zip_file", 
    params(
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint"),
        ("project_name" = String, Query, description = "Name of the project"),
        ("google_drive_share_link" = String, Query, description = "Google drive share link for the zip file", example = "https://drive.google.com/file/d/1FAjgIAL81UvshCn2owqlcPnvXl_k0cP2/view?usp=sharing")
    ),
    tag = "task",
    responses(
        (status = 201, description = "Task was scheduled for running", body = DownloadZipFileOkReponse, example = json!(DownloadZipFileOkReponse{id: String::from("some-id")})),
        (status = 400, description = "Chat id missing, Api key missing"),
        (status = 401, description = "Api key invalid"),
    ),
    security(
        ("api_key" = []),
    ),
)]
pub async fn download_zip_file(
    State(state): State<ApiState>,
    ChatId(chat_id): ChatId,
    Query(query): Query<DownloadZipFileQuery>,
) -> Result<DownloadZipFileOkReponse, DownloadZipFileErrorReponse> {
    let project_name = query.project_name;
    let google_drive_share_link = query.google_drive_share_link;

    let google_drive_share_link =
        url::Url::parse(&google_drive_share_link).map_err(|_| DownloadZipFileErrorReponse {
            message: "Invalid url".to_string(),
        })?;

    let download_url = crate::server::utils::convert_google_share_or_view_url_to_download_url(
        google_drive_share_link,
    )
    .map_err(|e| DownloadZipFileErrorReponse {
        message: e.to_string(),
    })?;

    // TODO!
    let id = state.run_task(chat_id).await;

    Ok(DownloadZipFileOkReponse { id })
}
