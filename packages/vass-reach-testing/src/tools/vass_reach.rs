use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use vass_reach_lib::solver::{SerializableSolverResult, vass_reach::VASSReachSolverStatistics};

use crate::{
    config::{TestConfig, TestRunConfig, ToolConfig},
    testing::SolverRunResult,
    tools::Tool,
};

#[derive(Debug, Clone)]
pub struct VASSReachTool<'a> {
    tool_config: &'a ToolConfig,
    test_config: &'a TestConfig,
    test_path: PathBuf,
}

impl<'a> VASSReachTool<'a> {
    pub fn new(
        tool_config: &'a ToolConfig,
        test_config: &'a TestConfig,
        test_path: PathBuf,
    ) -> Self {
        Self {
            tool_config,
            test_config,
            test_path,
        }
    }
}

impl<'a> Tool for VASSReachTool<'a> {
    fn name(&self) -> &str {
        "vass-reach"
    }

    fn tool_config(&self) -> &ToolConfig {
        self.tool_config
    }

    fn test_config(&self) -> &TestConfig {
        self.test_config
    }

    fn test(&self) -> anyhow::Result<()> {
        Command::new(self.workspace_binary_path()?)
            .args(["--help"])
            .current_dir(self.workspace_root()?)
            .status()?;

        Ok(())
    }

    fn build(&self) -> anyhow::Result<()> {
        Command::new("cargo")
            .args(["build", "--release", "-p", "vass-reach"])
            .current_dir(self.workspace_root()?)
            .status()?;

        Ok(())
    }

    fn run_on_file(
        &self,
        file_path: &std::path::Path,
        config: &TestRunConfig,
    ) -> anyhow::Result<SolverRunResult> {
        let config_override_path = self.create_temp_config_with_trace(file_path, config)?;

        // `systemd-run --user --scope --unit=kreach_run_{file_stub} -p MemoryMax=4G -p
        // RuntimeMaxSec={self.test_config.timeout} ./target/release/vass-reach
        // {file_path}`
        let mut command = Command::new("systemd-run");
        let binary_path = self.workspace_binary_path()?;
        let unit_suffix = file_path
            .file_stem()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "instance".to_string());
        command
            .arg("--user")
            .arg("--scope")
            .arg(format!("--unit=vass-reach_run_{unit_suffix}"))
            .arg(format!("-pMemoryMax={}G", 4))
            .arg(format!("-pRuntimeMaxSec={}", self.test_config.timeout))
            .arg(binary_path.as_os_str())
            .arg(file_path.as_os_str())
            .arg(format!("-c={}", config_override_path.display()));
        command.current_dir(self.workspace_root()?);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let output = command.output()?;
        let _ = fs::remove_file(&config_override_path);

        // let mut command = Command::new("./target/release/vass-reach");
        // command.args(&[
        //     &format!("-t={}", self.test_config.timeout),
        //     file_path.to_str().unwrap()
        // ]);
        // command.current_dir(self.get_tool_path()?);
        // command.stdout(Stdio::piped());
        // command.stderr(Stdio::piped());

        // // the tool itself has a timeout, we give it some extra time to stop
        // gracefully before we kill it let command_timeout =
        // (self.test_config.timeout as f64 * 1.5) as u64; let output =
        // run_with_watcher(&mut command, command_timeout)?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let json_payload = extract_json_payload(&stdout).with_context(|| {
                format!(
                    "failed to extract solver JSON output from stdout:\n{}",
                    stdout
                )
            })?;
            let res: SerializableSolverResult<VASSReachSolverStatistics> =
                serde_json::from_str(json_payload).context("failed to parse solver JSON output")?;
            Ok(SolverRunResult::Success(res.to_empty_status()))
        } else {
            println!("Process exited with status: {}", output.status);
            let stderr = String::from_utf8_lossy(&output.stderr);

            // signal SIGTERM
            if output.status.code() == Some(15) || output.status.code() == Some(143) {
                // `systemctl show --user bar.scope`
                // TODO: use above command to parse termination reason
            }

            Ok(SolverRunResult::Crash(format!(
                "Process exited with status code {} and stderr:\n {}",
                output.status, stderr
            )))
        }
    }

    fn supports_instance_file(&self, file_path: &std::path::Path) -> bool {
        file_path.extension().and_then(|ext| ext.to_str()) == Some("spec")
            || is_vass_json_file(file_path)
    }
}

fn extract_json_payload(output: &str) -> anyhow::Result<&str> {
    for (start, _) in output.match_indices('{') {
        let candidate = &output[start..];
        let Some(end) = find_matching_json_object_end(candidate) else {
            continue;
        };

        let payload = &candidate[..end];
        if serde_json::from_str::<serde_json::Value>(payload).is_ok() {
            return Ok(payload);
        }
    }

    anyhow::bail!("no valid JSON object found in solver output")
}

fn find_matching_json_object_end(input: &str) -> Option<usize> {
    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in input.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }

            if ch == '\\' {
                escaped = true;
                continue;
            }

            if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    return Some(idx + ch.len_utf8());
                }
            }
            _ => {}
        }
    }

    None
}

fn is_vass_json_file(file_path: &Path) -> bool {
    file_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".vass.json"))
}

impl<'a> VASSReachTool<'a> {
    fn workspace_root(&self) -> anyhow::Result<PathBuf> {
        let mut current = std::fs::canonicalize(self.get_tool_path()?)?;

        loop {
            let cargo_toml = current.join("Cargo.toml");
            if cargo_toml.exists() {
                let content = fs::read_to_string(&cargo_toml)?;
                if content.contains("[workspace]") {
                    return Ok(current);
                }
            }

            let Some(parent) = current.parent() else {
                break;
            };
            current = parent.to_path_buf();
        }

        Err(anyhow::anyhow!(
            "failed to locate workspace root from tool path {}",
            self.get_tool_path()?.display()
        ))
    }

    fn workspace_binary_path(&self) -> anyhow::Result<PathBuf> {
        Ok(self
            .workspace_root()?
            .join("target")
            .join("release")
            .join("vass-reach"))
    }

    fn create_temp_config_with_trace(
        &self,
        file_path: &std::path::Path,
        run_config: &TestRunConfig,
    ) -> anyhow::Result<PathBuf> {
        let base_config_path = self.test_path.join(&run_config.config);
        let base_content = fs::read_to_string(&base_config_path)?;
        let mut value: toml::Value = toml::from_str(&base_content)?;

        let value_table = value
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("vass-reach config root must be a TOML table"))?;

        let instance_name = file_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "instance".to_string());

        let debug_root = self.test_path.join("debug-traces").join("vass-reach");
        let instance_trace_dir = debug_root.join(&run_config.name).join(&instance_name);

        // Ensure re-runs do not mix old and new trace steps for the same run and
        // instance.
        if instance_trace_dir.exists() {
            fs::remove_dir_all(&instance_trace_dir)?;
        }

        let mut debug_trace = toml::map::Map::new();
        debug_trace.insert("enabled".to_string(), toml::Value::Boolean(true));
        debug_trace.insert(
            "output_root".to_string(),
            toml::Value::String(debug_root.display().to_string()),
        );
        debug_trace.insert(
            "run_name".to_string(),
            toml::Value::String(run_config.name.clone()),
        );
        debug_trace.insert(
            "instance_name".to_string(),
            toml::Value::String(instance_name),
        );

        value_table.insert("debug_trace".to_string(), toml::Value::Table(debug_trace));

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp_path = std::env::temp_dir().join(format!(
            "vass_reach_test_config_{}_{}.toml",
            std::process::id(),
            unique
        ));

        fs::write(&temp_path, toml::to_string(&value)?)?;
        Ok(temp_path)
    }
}
