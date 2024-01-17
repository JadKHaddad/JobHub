use anyhow::Context;
use axum::{
    http::Method,
    routing::{get, post, put},
    Router,
};
use job_hub::{openapi::ApiDoc, routes, server::state::ServerState};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use utoipa::OpenApi;
use utoipa_rapidoc::RapiDoc;
use utoipa_redoc::{Redoc, Servable};
use utoipa_swagger_ui::SwaggerUi;

fn init_tracing() -> anyhow::Result<()> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt::Subscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish(),
    )
    .context("Failed to set global tracing subscriber")?;

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "job_hub=debug,tower_http=trace");
    }

    init_tracing()?;

    let state = ServerState::new();

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(Redoc::with_url("/redoc", ApiDoc::openapi()))
        .merge(RapiDoc::new("/api-docs/openapi.json").path("/rapidoc"))
        .route("/health", get(|| async { "health" }))
        .route("/run", post(routes::run::run))
        .route("/kill/:id", put(routes::kill::kill))
        .route("/status/:id", get(routes::status::status))
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
                        .on_request(DefaultOnRequest::new().level(tracing::Level::INFO))
                        .on_response(DefaultOnResponse::new().level(tracing::Level::INFO)),
                )
                .layer(
                    CorsLayer::new()
                        .allow_origin(tower_http::cors::Any)
                        .allow_methods([Method::GET, Method::OPTIONS, Method::POST]),
                ),
        );

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 3000);

    tracing::info!(%addr, "Starting server");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context("Bind failed")?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("Server failed")?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutting down");
}
