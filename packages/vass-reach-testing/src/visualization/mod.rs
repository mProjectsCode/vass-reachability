use std::{fs, process::Command, sync::Arc};

use axum::{
    Json, Router, extract::State, http::{HeaderValue, Method, StatusCode},
    routing::{get, post},
};
use tower_http::cors::{Any, CorsLayer};
use vass_reach_lib::logger::Logger;

use crate::{
    Args,
    config::{CustomError, Test, TestData, UIConfig, load_ui_config},
};

pub fn visualize(logger: &Logger, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let ui_config = load_ui_config()?;

    start_ui(&ui_config)?;

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(start_server(logger, args, ui_config))
}

async fn start_server(
    logger: &Logger,
    args: &Args,
    ui_config: UIConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = Arc::new(ui_config);

    let cors_layer = CorsLayer::new()
        .allow_origin(format!("http://localhost:{}", &config.ui_port).parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/list_test_folders", get(list_test_folders_handler))
        .route("/api/test_data", post(test_data_handler))
        .with_state(Arc::clone(&config))
        .layer(cors_layer);

    let addr = format!("0.0.0.0:{}", config.server_port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    axum::serve(listener, app).await.unwrap();

    logger.info("after server await");

    Ok(())
}

fn start_ui(ui_config: &UIConfig) -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Command::new("bun");
    command.args(&[
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

fn handle_error(err: Box<dyn std::error::Error>) -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Something went wrong: {err}"),
    )
}

async fn list_test_folders_handler(
    State(config): State<Arc<UIConfig>>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    match list_test_folders_inner(config).await {
        Ok(x) => Ok(x.into()),
        Err(e) => Err(handle_error(e)),
    }
}

async fn list_test_folders_inner(
    config: Arc<UIConfig>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let folder = fs::canonicalize(&config.test_folders_path)?;
    Ok(folder
        .read_dir()?
        .filter_map(|f| f.ok().map(|f| f.path()))
        .filter(|f| f.is_dir())
        .filter_map(|f| f.to_str().map(|s| s.to_string()))
        .collect::<Vec<_>>())
}

async fn test_data_handler(
    State(config): State<Arc<UIConfig>>,
    Json(folder): Json<String>,
) -> Result<Json<TestData>, (StatusCode, String)> {
    println!("Handler");
    match test_data_inner(folder, config).await {
        Ok(x) => Ok(Json(x)),
        Err(e) => Err(handle_error(e)),
    }
}

async fn test_data_inner(
    folder: String,
    config: Arc<UIConfig>,
) -> Result<TestData, Box<dyn std::error::Error>> {
    let test = Test::from_string(folder)?;

    if !test.is_inside_folder(&config.test_folders_path)? {
        return CustomError::str("Test folder is not in configured test folder").to_boxed();
    }

    test.try_into()
}