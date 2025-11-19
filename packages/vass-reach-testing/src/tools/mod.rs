use enum_dispatch::enum_dispatch;

use crate::{config::{TestConfig, ToolConfig}, testing::SolverRunResult, tools::{kreach::KReachTool, vass_reach::VASSReachTool}};

pub mod vass_reach;
pub mod kreach;

#[enum_dispatch(ToolWrapper)]
pub trait Tool {
    fn name(&self) -> &str;
    fn tool_config(&self) -> &ToolConfig;
    fn test_config(&self) -> &TestConfig;
    fn test(&self) -> Result<(), Box<dyn std::error::Error>>;
    fn build(&self) -> Result<(), Box<dyn std::error::Error>>;
    fn run_on_file(&self, file_path: &std::path::Path) -> Result<SolverRunResult, Box<dyn std::error::Error>>;

    fn get_tool_path(&self) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        match self.tool_config().get(self.name()) {
            Some(path) => Ok(path.clone()),
            None => Err(format!("Tool {} not found in tool configuration", self.name()).into()),
        }
    }
}

#[enum_dispatch]
pub enum ToolWrapper<'a> {
    VASSReach(VASSReachTool<'a>),
    KReach(KReachTool<'a>),
}