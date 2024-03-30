//! TODO: Error responses are overlapping
//! TODO: Impl intoSchema for ApiError or something
//! TODO: Use some derives for Query paramas. Descriptions are getting out of control
//!
use utoipa::{
    openapi::{
        security::{ApiKey, ApiKeyValue, SecurityScheme},
        OpenApi as OpenApiDoc, OpenApiBuilder, Server,
    },
    OpenApi,
};

// TODO: Error responses are added to the schema, but they are not referenced in the paths
#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::gs_log_to_locst_converter::gs_log_to_locst_converter,
        crate::routes::cancel::cancel,
        crate::routes::status::status,
        crate::routes::request_chat_id::request_chat_id,
        crate::routes::upload_zip_file::download_zip_file,
        crate::routes::log_files::list_log_files,
        crate::routes::log_files::get_log_file_text,
    ),
    components(schemas(
        crate::server::task::Status,
        crate::server::task::DownloadZipFileStatus,
        crate::server::task::ProcessStatus,
        crate::server::task::FailOperation,
        crate::server::task::ExitedStatus,
        crate::routes::gs_log_to_locst_converter::GsLogToLocstConverterOkResponse,
        crate::routes::gs_log_to_locst_converter::GsLogToLocstConverterErrorResponse,
        crate::routes::cancel::CancelOkReponse,
        crate::routes::cancel::CancelErrorReponse,
        crate::routes::status::StatusOkReponse,
        crate::routes::status::StatusErrorReponse,
        crate::routes::request_chat_id::RequestChatIdReponse,
        crate::routes::upload_zip_file::DownloadZipFileOkReponse,
        crate::routes::upload_zip_file::DownloadZipFileErrorReponse,
        crate::routes::log_files::ListLogfilesOkResponse,
        crate::routes::log_files::ListLogfilesErrorResponse,
        crate::routes::log_files::GetLogFileErrorResponse,
    ))
)]
struct ApiDoc;

pub fn build_openapi(server_urls: Vec<String>) -> OpenApiDoc {
    let openapi: OpenApiDoc = ApiDoc::openapi();

    let components = openapi.components.map(|mut components| {
        components.add_security_scheme(
            "api_key",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("api_key"))),
        );
        components
    });

    OpenApiBuilder::new()
        .paths(openapi.paths)
        .components(components)
        .servers(Some(
            server_urls.into_iter().map(Server::new).collect::<Vec<_>>(),
        ))
        .build()
}
