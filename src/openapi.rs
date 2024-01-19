use utoipa::{
    openapi::{
        security::{ApiKey, ApiKeyValue, SecurityScheme},
        OpenApi as OpenApiDoc, OpenApiBuilder, Server,
    },
    Modify, OpenApi,
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
        crate::routes::run::RunReponse,
        crate::routes::cancel::CancelReponse,
        crate::routes::status::StatusReponse,
    )),
    modifiers(&SecurityAddon),
)]
struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "api_key",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("api_key"))),
            )
        }
    }
}

pub fn build_openapi(public_domain_urls: Vec<String>) -> OpenApiDoc {
    let openapi = ApiDoc::openapi();

    OpenApiBuilder::new()
        .paths(openapi.paths)
        .components(openapi.components)
        .security(openapi.security)
        .servers(Some(
            public_domain_urls
                .into_iter()
                .map(Server::new)
                .collect::<Vec<_>>(),
        ))
        .build()
}
