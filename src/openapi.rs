use utoipa::{
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
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
    servers(
        (url = "http://localhost:3000"), 
    ),
    modifiers(&SecurityAddon),
)]
pub struct ApiDoc;

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
