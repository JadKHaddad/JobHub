use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::run::run,
        crate::routes::kill::kill,
        crate::routes::status::status,
    ),
    components(schemas(
        crate::server::task::Status,
        crate::routes::run::RunReponse,
        crate::routes::kill::KillReponse,
        crate::routes::status::StatusReponse,
    )),
    servers(
        (url = "http://134.122.85.124:3000"), 
    )
)]
pub struct ApiDoc;
