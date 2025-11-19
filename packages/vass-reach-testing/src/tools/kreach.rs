use std::process::{Command, Stdio};

use regex::Regex;
use vass_reach_lib::solver::{SerializableSolverResult, SerializableSolverStatus};

use crate::{
    config::{TestConfig, ToolConfig}, testing::SolverRunResult, tools::Tool
};

#[derive(Debug, Clone)]
pub struct KReachTool<'a> {
    tool_config: &'a ToolConfig,
    test_config: &'a TestConfig,
}

impl<'a> KReachTool<'a> {
    pub fn new(tool_config: &'a ToolConfig, test_config: &'a TestConfig) -> Self {
        Self {
            tool_config,
            test_config,
        }
    }
}

impl<'a> Tool for KReachTool<'a> {
    fn name(&self) -> &str {
        "kreach"
    }

    fn tool_config(&self) -> &ToolConfig {
        self.tool_config
    }

    fn test_config(&self) -> &TestConfig {
        self.test_config
    }

    fn test(&self) -> Result<(), Box<dyn std::error::Error>> {
        Command::new("stack")
            .args(&["exec", "kosaraju"])
            .current_dir(self.get_tool_path()?)
            .status()?;

        Ok(())
    }

    fn build(&self) -> Result<(), Box<dyn std::error::Error>> {
        Command::new("stack")
            .args(&["build", "kosaraju"])
            .current_dir(self.get_tool_path()?)
            .status()?;

        Ok(())
    }

    fn run_on_file(
        &self,
        file_path: &std::path::Path,
    ) -> Result<SolverRunResult, Box<dyn std::error::Error>> {
        // `systemd-run --user --scope --unit=kreach_run_{file_stub} -p MemoryMax=4G -p RuntimeMaxSec={self.test_config.timeout} stack exec kosaraju -- -r {file_path}`
        let mut command = Command::new("systemd-run");
        command.args(&[
            "--user",
            "--scope",
            &format!("--unit=kreach_run_{}", file_path.file_stem().unwrap().to_str().unwrap()),
            &format!("-pMemoryMax={}G", 4),
            &format!("-pRuntimeMaxSec={}", self.test_config.timeout),
            "stack",
            "exec", 
            "kosaraju", 
            "--", 
            "-r", 
            file_path.to_str().unwrap()
        ]);
        command.current_dir(self.get_tool_path()?);
        command.env("KOSARAJU_SOLVER", "cvc4");
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let output = command.output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);

            let reachable_regexp = Regex::new(r"\sReachable\s").unwrap();
            let unreachable_regexp = Regex::new(r"\sUnreachable\s").unwrap();

            let res = if reachable_regexp.is_match(&stdout) {
                SerializableSolverStatus::True
            } else if unreachable_regexp.is_match(&stdout) {
                SerializableSolverStatus::False
            } else {
                SerializableSolverStatus::Unknown
            };

            Ok(SolverRunResult::Success(SerializableSolverResult::new(
                res,
                (),
            )))
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
