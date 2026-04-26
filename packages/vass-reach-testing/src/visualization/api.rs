use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;
use vass_reach_lib::solver::vass_reach::debug_trace::{
    DerivedSCCMetadata, StepTraceSeed, TraceStepSccViewSeed, derive_scc_component_view,
    derive_scc_metadata,
};

use super::{
    handle_error,
    trace_store::{
        TraceRunInfo, list_test_folders_inner, list_trace_steps_inner, list_traces_inner,
        test_data_inner, trace_step_seed_inner,
    },
};
use crate::config::{TestData, UIConfig};

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TraceStepRequest {
    pub folder: String,
    pub run_name: String,
    pub instance_name: String,
    pub step: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TraceStepListRequest {
    pub folder: String,
    pub run_name: String,
    pub instance_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TraceStepSccRequest {
    pub folder: String,
    pub run_name: String,
    pub instance_name: String,
    pub step: u64,
    pub component_index: usize,
}

pub(crate) async fn list_test_folders_handler(
    State(config): State<Arc<UIConfig>>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    match list_test_folders_inner(config).await {
        Ok(x) => Ok(x.into()),
        Err(e) => Err(handle_error(e)),
    }
}

pub(crate) async fn test_data_handler(
    State(config): State<Arc<UIConfig>>,
    Json(folder): Json<String>,
) -> Result<Json<TestData>, (StatusCode, String)> {
    match test_data_inner(folder, config).await {
        Ok(x) => Ok(Json(x)),
        Err(e) => Err(handle_error(e)),
    }
}

pub(crate) async fn list_traces_handler(
    State(config): State<Arc<UIConfig>>,
    Json(folder): Json<String>,
) -> Result<Json<Vec<TraceRunInfo>>, (StatusCode, String)> {
    match list_traces_inner(folder, config).await {
        Ok(x) => Ok(Json(x)),
        Err(e) => Err(handle_error(e)),
    }
}

pub(crate) async fn trace_step_seed_handler(
    State(config): State<Arc<UIConfig>>,
    Json(req): Json<TraceStepRequest>,
) -> Result<Json<StepTraceSeed>, (StatusCode, String)> {
    match trace_step_seed_inner(
        req.folder,
        req.run_name,
        req.instance_name,
        req.step,
        config,
    )
    .await
    {
        Ok(x) => Ok(Json(x)),
        Err(e) => Err(handle_error(e)),
    }
}

pub(crate) async fn list_trace_steps_handler(
    State(config): State<Arc<UIConfig>>,
    Json(req): Json<TraceStepListRequest>,
) -> Result<Json<Vec<u64>>, (StatusCode, String)> {
    match list_trace_steps_inner(req.folder, req.run_name, req.instance_name, config).await {
        Ok(x) => Ok(Json(x)),
        Err(e) => Err(handle_error(e)),
    }
}

pub(crate) async fn trace_step_metadata_handler(
    State(config): State<Arc<UIConfig>>,
    Json(req): Json<TraceStepRequest>,
) -> Result<Json<DerivedSCCMetadata>, (StatusCode, String)> {
    match trace_step_seed_inner(
        req.folder,
        req.run_name,
        req.instance_name,
        req.step,
        config,
    )
    .await
    {
        Ok(seed) => Ok(Json(derive_scc_metadata(&seed.scc_dag))),
        Err(e) => Err(handle_error(e)),
    }
}

pub(crate) async fn trace_step_scc_view_handler(
    State(config): State<Arc<UIConfig>>,
    Json(req): Json<TraceStepSccRequest>,
) -> Result<Json<TraceStepSccViewSeed>, (StatusCode, String)> {
    match trace_step_seed_inner(
        req.folder,
        req.run_name,
        req.instance_name,
        req.step,
        config,
    )
    .await
    {
        Ok(seed) => match derive_scc_component_view(&seed, req.component_index) {
            Ok(view) => Ok(Json(view)),
            Err(e) => Err(handle_error(e)),
        },
        Err(e) => Err(handle_error(e)),
    }
}
