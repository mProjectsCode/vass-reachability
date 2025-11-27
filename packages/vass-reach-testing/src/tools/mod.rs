use enum_dispatch::enum_dispatch;

use crate::{
    config::{TestConfig, TestRunConfig, ToolConfig},
    testing::SolverRunResult,
    tools::{kreach::KReachTool, vass_reach::VASSReachTool},
};

pub mod kreach;
pub mod vass_reach;

#[enum_dispatch(ToolWrapper)]
pub trait Tool {
    fn name(&self) -> &str;
    fn tool_config(&self) -> &ToolConfig;
    fn test_config(&self) -> &TestConfig;
    fn test(&self) -> anyhow::Result<()>;
    fn build(&self) -> anyhow::Result<()>;
    fn run_on_file(
        &self,
        file_path: &std::path::Path,
        config: &TestRunConfig,
    ) -> anyhow::Result<SolverRunResult>;

    fn get_tool_path(&self) -> anyhow::Result<std::path::PathBuf> {
        match self.tool_config().get(self.name()) {
            Some(path) => Ok(path.clone()),
            None => Err(anyhow::anyhow!(
                "Tool {} not found in tool configuration",
                self.name()
            )),
        }
    }
}

#[enum_dispatch]
pub enum ToolWrapper<'a> {
    VASSReach(VASSReachTool<'a>),
    KReach(KReachTool<'a>),
}
