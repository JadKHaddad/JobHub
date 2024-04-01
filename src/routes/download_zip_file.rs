use crate::server::{
    extractors::{chat_id::ChatId, query::Query},
    response::ApiError,
    state::ApiState,
    utils::GoogleConvertLinkError,
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
pub struct DownloadZipFileOkReponse {
    /// Task id that was scheduled for running
    #[schema(example = "0")]
    id: String,
}

#[derive(Serialize, ToSchema)]
#[serde(tag = "error", content = "content")]
pub enum DownloadZipFileErrorReponse {
    InvalidUrl,
    Convert(GoogleConvertLinkError),
    ServerError(ApiError),
}

impl IntoResponse for DownloadZipFileOkReponse {
    fn into_response(self) -> Response {
        (StatusCode::CREATED, Json(self)).into_response()
    }
}

impl IntoResponse for DownloadZipFileErrorReponse {
    fn into_response(self) -> Response {
        match self {
            DownloadZipFileErrorReponse::InvalidUrl => {
                (StatusCode::BAD_REQUEST, Json(self)).into_response()
            }
            DownloadZipFileErrorReponse::Convert(_) => {
                (StatusCode::BAD_REQUEST, Json(self)).into_response()
            }
            DownloadZipFileErrorReponse::ServerError(err) => err.into_response(),
        }
    }
}

#[derive(Deserialize)]
pub struct DownloadZipFileQuery {
    /// Name of the project
    project_name: String,
    /// Google drive share link for the zip file
    google_drive_share_link: String,
}

/// Schedule a download of a zip file from a Google Drive link.
///
/// This endpoint will schedule a task for running. The task will be executed asynchronously.
#[utoipa::path(
    post,
    path = "/api/download_zip_file", 
    params(
        ("chat_id" = String, Query, description = "Chat id. generated using the `/api/request_chat_id` endpoint."),
        ("project_name" = String, Query, description = "Name of the project."),
        ("google_drive_share_link" = String, Query, description = "Google drive share link for the zip file.")
    ),
    tag = "download",
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

    let google_drive_share_link = url::Url::parse(&google_drive_share_link)
        .map_err(|_| DownloadZipFileErrorReponse::InvalidUrl)?;

    let download_url = crate::server::utils::convert_google_share_or_view_url_to_download_url(
        google_drive_share_link,
    )
    .map_err(DownloadZipFileErrorReponse::Convert)?;

    let id = state
        .run_download_task(chat_id, download_url, project_name)
        .await
        .map_err(|err| DownloadZipFileErrorReponse::ServerError(err.into()))?;

    Ok(DownloadZipFileOkReponse { id })
}
