use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

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
        Command::new("./target/release/vass-reach")
            .args(&["--help"])
            .current_dir(self.get_tool_path()?)
            .status()?;

        Ok(())
    }

    fn build(&self) -> anyhow::Result<()> {
        Command::new("cargo")
            .args(&["build", "--release"])
            .current_dir(self.get_tool_path()?)
            .status()?;

        Ok(())
    }

    fn run_on_file(
        &self,
        file_path: &std::path::Path,
        config: &TestRunConfig,
    ) -> anyhow::Result<SolverRunResult> {
        // `systemd-run --user --scope --unit=kreach_run_{file_stub} -p MemoryMax=4G -p RuntimeMaxSec={self.test_config.timeout} ./target/release/vass-reach {file_path}`
        let mut command = Command::new("systemd-run");
        command.args(&[
            "--user",
            "--scope",
            &format!(
                "--unit=vass-reach_run_{}",
                file_path.file_stem().unwrap().to_str().unwrap()
            ),
            &format!("-pMemoryMax={}G", 4),
            &format!("-pRuntimeMaxSec={}", self.test_config.timeout),
            "./target/release/vass-reach",
            file_path.to_str().unwrap(),
            &format!("-c={}", self.test_path.join(&config.config).display()),
        ]);
        command.current_dir(self.get_tool_path()?);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let output = command.output()?;

        // let mut command = Command::new("./target/release/vass-reach");
        // command.args(&[
        //     &format!("-t={}", self.test_config.timeout),
        //     file_path.to_str().unwrap()
        // ]);
        // command.current_dir(self.get_tool_path()?);
        // command.stdout(Stdio::piped());
        // command.stderr(Stdio::piped());

        // // the tool itself has a timeout, we give it some extra time to stop gracefully before we kill it
        // let command_timeout = (self.test_config.timeout as f64 * 1.5) as u64;
        // let output = run_with_watcher(&mut command, command_timeout)?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let res: SerializableSolverResult<VASSReachSolverStatistics> =
                serde_json::from_str(&stdout)?;
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
                output.status,
                stderr.to_string()
            )))
        }
    }
}
