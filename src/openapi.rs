use utoipa::{
    openapi::{
        security::{ApiKey, ApiKeyValue, SecurityScheme},
        OpenApi as OpenApiDoc, OpenApiBuilder, SecurityRequirement, Server,
    },
    OpenApi,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::run::run,
        crate::routes::cancel::cancel,
        crate::routes::status::status,
    ),
    components(schemas(
        crate::server::task::Status,
        crate::server::task::FailOperation,
        crate::server::task::ExitedStatus,
        crate::routes::run::RunReponse,
        crate::routes::cancel::CancelReponse,
        crate::routes::status::StatusReponse,
    ))
)]
struct ApiDoc;

pub fn build_openapi(server_urls: Vec<String>) -> OpenApiDoc {
    let openapi: OpenApiDoc = ApiDoc::openapi();

    // All paths require authentication with api_key
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
        .security(Some(vec![SecurityRequirement::new(
            "api_key",
            ["edit:items", "read:items"],
        )]))
        .build()
}
