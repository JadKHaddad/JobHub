use utoipa::OpenApi;

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
        (url = "http://134.122.85.124:3000"), 
    )
)]
pub struct ApiDoc;
