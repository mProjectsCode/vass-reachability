use std::{fs, path::PathBuf, sync::Arc};

use anyhow::Context;
use serde::Serialize;
use vass_reach_lib::solver::vass_reach::debug_trace::StepTraceSeed;

use crate::config::{Test, TestData, UIConfig};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TraceRunInfo {
    pub run_name: String,
    pub instances: Vec<String>,
}

pub(crate) async fn list_test_folders_inner(config: Arc<UIConfig>) -> anyhow::Result<Vec<String>> {
    let folder = fs::canonicalize(&config.test_folders_path)?;
    Ok(folder
        .read_dir()?
        .filter_map(|f| f.ok().map(|f| f.path()))
        .filter(|f| f.is_dir())
        .filter_map(|f| f.to_str().map(|s| s.to_string()))
        .collect::<Vec<_>>())
}

pub(crate) async fn test_data_inner(
    folder: String,
    config: Arc<UIConfig>,
) -> anyhow::Result<TestData> {
    let test = Test::from_string(folder)?;

    if !test.is_inside_folder(&config.test_folders_path)? {
        anyhow::bail!("Test folder is not in configured test folder");
    }

    test.try_into()
}

pub(crate) async fn list_traces_inner(
    folder: String,
    config: Arc<UIConfig>,
) -> anyhow::Result<Vec<TraceRunInfo>> {
    let test = Test::from_string(folder)?;
    if !test.is_inside_folder(&config.test_folders_path)? {
        anyhow::bail!("Test folder is not in configured test folder");
    }

    let trace_root = resolve_trace_root_for_test(&test.path);
    if !trace_root.exists() {
        return Ok(vec![]);
    }

    let mut runs = vec![];
    for run_entry in fs::read_dir(&trace_root)? {
        let run_entry = run_entry?;
        let run_path = run_entry.path();
        if !run_path.is_dir() {
            continue;
        }

        let Some(run_name) = run_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
        else {
            continue;
        };

        let mut instances = vec![];
        for instance_entry in fs::read_dir(&run_path)? {
            let instance_entry = instance_entry?;
            let instance_path = instance_entry.path();
            if !instance_path.is_dir() {
                continue;
            }
            if let Some(instance_name) = instance_path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
            {
                instances.push(instance_name);
            }
        }

        instances.sort();
        runs.push(TraceRunInfo {
            run_name,
            instances,
        });
    }

    runs.sort_by(|left, right| left.run_name.cmp(&right.run_name));
    Ok(runs)
}

pub(crate) async fn trace_step_seed_inner(
    folder: String,
    run_name: String,
    instance_name: String,
    step: u64,
    config: Arc<UIConfig>,
) -> anyhow::Result<StepTraceSeed> {
    let step_file = resolve_step_trace_file(&folder, &run_name, &instance_name, step, &config)?;
    let content = fs::read_to_string(step_file)?;
    Ok(serde_json::from_str(&content)?)
}

pub(crate) async fn list_trace_steps_inner(
    folder: String,
    run_name: String,
    instance_name: String,
    config: Arc<UIConfig>,
) -> anyhow::Result<Vec<u64>> {
    let test = Test::from_string(folder)?;
    if !test.is_inside_folder(&config.test_folders_path)? {
        anyhow::bail!("Test folder is not in configured test folder");
    }

    let trace_root = resolve_trace_root_for_test(&test.path);
    let canonical_root = fs::canonicalize(&trace_root).with_context(|| {
        format!(
            "failed to resolve trace root for test folder: {}",
            test.path.display()
        )
    })?;

    let steps_dir = trace_root.join(run_name).join(instance_name).join("steps");
    let canonical_steps_dir = fs::canonicalize(&steps_dir)
        .with_context(|| format!("failed to resolve steps directory: {}", steps_dir.display()))?;

    if !canonical_steps_dir.starts_with(&canonical_root) {
        anyhow::bail!("Resolved steps directory is outside debug trace root");
    }

    let mut steps = vec![];
    for entry in fs::read_dir(canonical_steps_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().map(|name| name.to_string_lossy()) else {
            continue;
        };

        if !file_name.starts_with("step_") || !file_name.ends_with(".json") {
            continue;
        }

        let number = file_name
            .strip_prefix("step_")
            .and_then(|name| name.strip_suffix(".json"))
            .and_then(|name| name.parse::<u64>().ok());
        if let Some(number) = number {
            steps.push(number);
        }
    }

    steps.sort_unstable();
    Ok(steps)
}

fn resolve_step_trace_file(
    folder: &str,
    run_name: &str,
    instance_name: &str,
    step: u64,
    config: &UIConfig,
) -> anyhow::Result<PathBuf> {
    let test = Test::from_string(folder.to_string())?;
    if !test.is_inside_folder(&config.test_folders_path)? {
        anyhow::bail!("Test folder is not in configured test folder");
    }

    let trace_root = resolve_trace_root_for_test(&test.path);
    let canonical_root = fs::canonicalize(&trace_root).with_context(|| {
        format!(
            "failed to resolve trace root for test folder: {}",
            test.path.display()
        )
    })?;

    let step_file = trace_root
        .join(run_name)
        .join(instance_name)
        .join("steps")
        .join(format!("step_{step:06}.json"));

    let canonical_step_file = fs::canonicalize(&step_file)
        .with_context(|| format!("failed to resolve step trace file: {}", step_file.display()))?;

    if !canonical_step_file.starts_with(&canonical_root) {
        anyhow::bail!("Resolved step trace path is outside debug trace root");
    }

    Ok(canonical_step_file)
}

fn debug_trace_root_for_test(test_folder: &std::path::Path) -> std::path::PathBuf {
    test_folder.join("debug-traces").join("vass-reach")
}

fn legacy_debug_trace_root_for_test(test_folder: &std::path::Path) -> std::path::PathBuf {
    test_folder
        .parent()
        .unwrap_or(test_folder)
        .join("debug-traces")
        .join("vass-reach")
}

fn resolve_trace_root_for_test(test_folder: &std::path::Path) -> std::path::PathBuf {
    let current = debug_trace_root_for_test(test_folder);
    if current.exists() {
        return current;
    }

    let legacy = legacy_debug_trace_root_for_test(test_folder);
    if legacy.exists() {
        return legacy;
    }

    current
}
