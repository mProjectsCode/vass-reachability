use std::{fs, process::Command, sync::Arc};

use anyhow::Context;
use axum::{
    Router,
    http::{HeaderValue, Method, StatusCode},
    routing::{get, post},
};
use tower_http::cors::{Any, CorsLayer};

use crate::{
    Args,
    config::{UIConfig, load_ui_config},
};

mod api;
mod trace_store;

pub fn visualize(args: &Args) -> anyhow::Result<()> {
    let ui_config = load_ui_config()?;

    start_ui(&ui_config).context("failed to start ui")?;

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(start_server(args, ui_config))
        .context("failed to run server")
}

async fn start_server(_args: &Args, ui_config: UIConfig) -> anyhow::Result<()> {
    let config = Arc::new(ui_config);

    let cors_layer = CorsLayer::new()
        .allow_origin(
            format!("http://localhost:{}", &config.ui_port)
                .parse::<HeaderValue>()
                .unwrap(),
        )
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let app = Router::new()
        .route(
            "/api/list_test_folders",
            get(api::list_test_folders_handler),
        )
        .route("/api/test_data", post(api::test_data_handler))
        .route("/api/list_traces", post(api::list_traces_handler))
        .route("/api/list_trace_steps", post(api::list_trace_steps_handler))
        .route("/api/trace_step_seed", post(api::trace_step_seed_handler))
        .route(
            "/api/trace_step_metadata",
            post(api::trace_step_metadata_handler),
        )
        .route(
            "/api/trace_step_scc_view",
            post(api::trace_step_scc_view_handler),
        )
        .with_state(Arc::clone(&config))
        .layer(cors_layer);

    let addr = format!("0.0.0.0:{}", config.server_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    axum::serve(listener, app).await?;

    Ok(())
}

fn start_ui(ui_config: &UIConfig) -> anyhow::Result<()> {
    let mut command = Command::new("bun");
    command.args([
        "run",
        "dev",
        "--",
        "--",
        &format!("--server_port={}", ui_config.server_port),
        &format!("--ui_port={}", ui_config.ui_port),
    ]);
    command.current_dir(fs::canonicalize(&ui_config.ui_path)?);

    command.spawn()?;

    Ok(())
}

pub(crate) fn handle_error(err: anyhow::Error) -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Something went wrong: {err}"),
    )
}
