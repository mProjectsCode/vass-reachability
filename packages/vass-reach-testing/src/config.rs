use std::{
    error::Error,
    fmt::Display,
    fs,
    path::{self, Path, PathBuf},
};

use hashbrown::HashMap;
use serde::{Deserialize, Serialize};
use vass_reach_lib::automaton::petri_net::initialized::InitializedPetriNet;

use crate::{testing::SolverResultStatistic, tools::Tool};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Test {
    pub path: PathBuf,
}

impl Test {
    pub fn new(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        if !path.is_absolute() {
            return CustomError::str("Test path is not absolute").to_boxed();
        }

        Ok(Self { path })
    }

    pub fn from_string(path: String) -> Result<Self, Box<dyn std::error::Error>> {
        let path: PathBuf = path.into();

        if !path.is_absolute() {
            return CustomError::str("Test path is not absolute").to_boxed();
        }

        Ok(Self { path: path })
    }

    pub fn canonicalize<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            path: fs::canonicalize(path)?,
        })
    }

    pub fn is_inside_folder<P: AsRef<Path>>(
        &self,
        folder: P,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let canonical_folder = fs::canonicalize(folder)?;
        Ok(self.path.starts_with(canonical_folder))
    }

    pub fn test_config(&self) -> Result<TestConfig, Box<dyn std::error::Error>> {
        TestConfig::load_from_path(self.path.join("test.toml"))
    }

    pub fn instance_config(&self) -> Result<InstanceConfig, Box<dyn std::error::Error>> {
        InstanceConfig::load_from_path(self.path.join("instances.toml"))
    }

    pub fn instances_folder(&self) -> PathBuf {
        self.path.join("instances")
    }

    pub fn results_folder(&self) -> PathBuf {
        self.path.join("results")
    }

    pub fn write_results(
        &self,
        tool: &impl Tool,
        results: HashMap<String, SolverResultStatistic>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let results_folder = self.results_folder();
        if !results_folder.exists() {
            fs::create_dir_all(&results_folder)?
        }

        let results_file = results_folder.join(format!("{}.json", tool.name()));

        let tool_result = ToolResult::new(tool.name().to_string(), results);

        std::fs::write(&results_file, serde_json::to_string_pretty(&tool_result)?)?;

        Ok(())
    }

    pub fn write_nets(
        &self,
        nets: &Vec<InitializedPetriNet>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let instances_folder = self.instances_folder();
        if !instances_folder.exists() {
            fs::create_dir_all(&instances_folder)?
        }

        for (i, obj) in nets.iter().enumerate() {
            let file_path = instances_folder.join(format!("net_{i}.spec"));
            obj.to_spec_file(file_path.to_str().unwrap())?;
        }

        Ok(())
    }

    pub fn read_results(&self) -> Result<Vec<ToolResult>, Box<dyn std::error::Error>> {
        let results_folder = self.results_folder();
        let mut res = vec![];

        for entry in results_folder.read_dir()? {
            let entry = entry?;
            let entry_path = entry.path();

            if let Some(extension) = entry_path.extension()
                && extension == "json"
            {
                let content = fs::read_to_string(&entry_path)?;

                res.push(serde_json::from_str(&content)?);
            }
        }

        Ok(res)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestData {
    pub path: PathBuf,
    pub instance_config: InstanceConfig,
    pub test_config: TestConfig,
    pub tool_results: Vec<ToolResult>,
}

impl TryFrom<Test> for TestData {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: Test) -> Result<Self, Self::Error> {
        Ok(TestData {
            path: value.path.clone(),
            instance_config: value.instance_config()?,
            test_config: value.test_config()?,
            tool_results: value.read_results()?,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestConfig {
    pub tools: Vec<String>,
    pub timeout: u64,
    pub memory_max_gb: u64,
}

impl TestConfig {
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstanceConfig {
    pub num_instances: usize,
    pub seed: u64,
    pub petri_net_counters: usize,
    pub petri_net_transitions: usize,
    pub petri_net_max_tokens_per_transition: usize,
    pub petri_net_no_guards: bool,
}

impl InstanceConfig {
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}

pub type ToolConfig = HashMap<String, path::PathBuf>;

pub fn load_tool_config() -> Result<ToolConfig, Box<dyn std::error::Error>> {
    let config_path = path::Path::new("./tools.config.toml");
    let canonic_path = fs::canonicalize(config_path)?;
    let content = fs::read_to_string(canonic_path)?;
    let config: HashMap<String, String> = toml::from_str(&content)?;
    Ok(config
        .into_iter()
        .map(|(k, v)| (k, path::PathBuf::from(v)))
        .collect())
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UIConfig {
    pub server_port: u32,
    pub ui_port: u32,
    pub ui_path: String,
    pub test_folders_path: String,
}

pub fn load_ui_config() -> Result<UIConfig, Box<dyn std::error::Error>> {
    let config_path = path::Path::new("./ui.config.toml");
    let canonic_path = fs::canonicalize(config_path)?;
    let content = fs::read_to_string(canonic_path)?;
    Ok(toml::from_str(&content)?)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolResult {
    pub tool_name: String,
    pub results: HashMap<String, SolverResultStatistic>,
}

impl ToolResult {
    pub fn new(tool_name: String, results: HashMap<String, SolverResultStatistic>) -> Self {
        Self { tool_name, results }
    }
}

#[derive(Debug, Clone)]
pub struct CustomError {
    message: String,
}

impl CustomError {
    pub fn new(message: String) -> Self {
        Self { message }
    }

    pub fn str(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }

    pub fn to_boxed<T>(self) -> Result<T, Box<dyn std::error::Error>> {
        Err(Box::new(self))
    }
}

impl From<String> for CustomError {
    fn from(value: String) -> Self {
        CustomError::new(value)
    }
}

impl From<&str> for CustomError {
    fn from(value: &str) -> Self {
        CustomError::str(value)
    }
}

impl Display for CustomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for CustomError {}
