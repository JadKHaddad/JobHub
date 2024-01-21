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
    ),
    components(schemas(
        crate::server::task::Status,
        crate::server::task::FailOperation,
        crate::server::task::ExitedStatus,
        crate::routes::run::RunReponse,
        crate::routes::cancel::CancelReponse,
        crate::routes::status::StatusReponse,
        crate::routes::request_chat_id::RequestChatIdReponse,
    ))
)]
struct ApiDoc;

// TODO: Squash all Error responses in this one function call to avoid duplication
pub fn build_openapi(server_urls: Vec<String>) -> OpenApiDoc {
    let openapi: OpenApiDoc = ApiDoc::openapi();

    let components = openapi.components.map(|mut components| {
        components.add_security_scheme(
            "api_key",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("api_key"))),
        );
        components.add_security_scheme(
            "chat_id",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("chat_id"))),
        );

        components
    });

    OpenApiBuilder::new()
        .paths(openapi.paths)
        .components(components)
        .servers(Some(
            server_urls.into_iter().map(Server::new).collect::<Vec<_>>(),
        ))
        // .security(Some(vec![SecurityRequirement::new(
        //     "api_key",
        //     ["edit:items", "read:items"],
        // )]))
        .build()
}
