use std::{net::SocketAddr, path::PathBuf};

use anyhow::Context;
use axum::{
    extract::{ConnectInfo, Request, State, WebSocketUpgrade},
    http::Method,
    middleware::{self, Next},
    response::IntoResponse,
    routing::{get, post, put},
    Router,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization, UserAgent},
    TypedHeader,
};
use clap::Parser;
use futures::StreamExt;
use job_hub::{
    cli_args::CliArgs,
    openapi::ApiDoc,
    routes,
    server::{response::ApiError, state::ApiState},
};
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
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
    dotenv::dotenv().ok();

    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "job_hub=debug,tower_http=trace");
    }

    init_tracing()?;

    let cli_args = CliArgs::parse();

    let state = ApiState::new(cli_args.api_token);

    let api = Router::new()
        // TODO: Create an extractor for this. From headers 'chat_id.
        .route("/request_chat_id", get(|| async { "chat_id" }))
        .route("/run", post(routes::run::run))
        .route("/cancel/:id", put(routes::cancel::cancel))
        .route("/status/:id", get(routes::status::status))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state, validate_bearer_token));

    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");

    let app = Router::new()
        .fallback_service(ServeDir::new(assets_dir).append_index_html_on_directories(true))
        .nest("/api", api)
        .route("/health", get(|| async { "ok" }))
        .route("/ws", get(ws_handler))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(Redoc::with_url("/redoc", ApiDoc::openapi()))
        .merge(RapiDoc::new("/api-docs/openapi.json").path("/rapidoc"))
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

    let addr = cli_args.socket_address;

    tracing::info!(%addr, "Starting server");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context("Bind failed")?;

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("Server failed")?;

    Ok(())
}

async fn validate_bearer_token(
    State(state): State<ApiState>,
    TypedHeader(Authorization(bearer)): TypedHeader<Authorization<Bearer>>,
    request: Request,
    next: Next,
) -> Result<impl IntoResponse, ApiError> {
    let token = bearer.token();

    if !state.api_token_valid(token) {
        tracing::warn!(%token, "Invalid bearer token");
        return Err(ApiError::Unauthorized);
    }

    let res = next.run(request).await;

    Ok(res)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };

    ws.on_upgrade(move |socket| async move {
        tracing::info!(?addr, %user_agent,  "Websocket connected");

        let (_, mut receiver) = socket.split();

        while let Some(Ok(msg)) = receiver.next().await {
            tracing::info!(?msg, ?addr, "Websocket message received");
        }

        tracing::info!(?addr, "Websocket closed");
    })
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
