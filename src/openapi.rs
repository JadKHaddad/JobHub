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

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::run::run,
        crate::routes::cancel::cancel,
        crate::routes::status::status,
        crate::routes::request_chat_id::request_chat_id,
        crate::routes::log_files::list_log_files,
        crate::routes::log_files::get_log_file,
    ),
    components(schemas(
        crate::server::task::Status,
        crate::server::task::FailOperation,
        crate::server::task::ExitedStatus,
        crate::routes::run::RunReponse,
        crate::routes::cancel::CancelReponse,
        crate::routes::status::StatusReponse,
        crate::routes::request_chat_id::RequestChatIdReponse,
        crate::routes::log_files::ListLogfilesResponse,
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
