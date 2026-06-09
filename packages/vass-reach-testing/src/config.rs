use std::{
    fs,
    path::{self, Path, PathBuf},
};

use anyhow::{Context, bail};
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};
use vass_reach_lib::automaton::petri_net::initialized::InitializedPetriNet;

use crate::{testing::SolverResultStatistic, tools::Tool};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Test {
    pub path: PathBuf,
}

impl Test {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        if !path.is_absolute() {
            bail!("Test path is not absolute");
        }

        Ok(Self { path })
    }

    pub fn from_string(path: String) -> anyhow::Result<Self> {
        let path: PathBuf = path.into();

        if !path.is_absolute() {
            bail!("Test path is not absolute");
        }

        Ok(Self { path })
    }

    pub fn canonicalize<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        Ok(Self {
            path: fs::canonicalize(path)?,
        })
    }

    pub fn is_inside_folder<P: AsRef<Path>>(&self, folder: P) -> anyhow::Result<bool> {
        let canonical_folder = fs::canonicalize(folder)?;
        Ok(self.path.starts_with(canonical_folder))
    }

    pub fn test_config(&self) -> anyhow::Result<TestConfig> {
        TestConfig::load_from_path(self.path.join("test.toml"))
    }

    pub fn instance_config(&self) -> anyhow::Result<InstanceConfig> {
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
        config: &TestRunConfig,
    ) -> anyhow::Result<()> {
        let results_folder = self.results_folder();
        if !results_folder.exists() {
            fs::create_dir_all(&results_folder)?
        }

        let results_file = results_folder.join(format!("{}.json", config.name));

        let tool_result = ToolResult::new(tool.name().to_string(), config.name.clone(), results);

        std::fs::write(&results_file, serde_json::to_string_pretty(&tool_result)?)?;

        Ok(())
    }

    pub fn write_nets(&self, nets: &[InitializedPetriNet]) -> anyhow::Result<()> {
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

    pub fn write_vass_instances(
        &self,
        instances: &[vass_reach_lib::automaton::vass::initialized::InitializedVASS<usize, usize>],
    ) -> anyhow::Result<()> {
        let instances_folder = self.instances_folder();
        if !instances_folder.exists() {
            fs::create_dir_all(&instances_folder)?
        }

        for (i, instance) in instances.iter().enumerate() {
            let file_path = instances_folder.join(format!("vass_{i}.vass.json"));
            instance.to_json_file(
                file_path
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("instance path is not valid UTF-8"))?,
            )?;
        }
        Ok(())
    }

    pub fn read_results(&self) -> anyhow::Result<Vec<ToolResult>> {
        let results_folder = self.results_folder();
        let mut res = vec![];
        if !results_folder.exists() {
            return Ok(res);
        }

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
    type Error = anyhow::Error;

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
    pub runs: Vec<TestRunConfig>,
    pub timeout: u64,
    pub memory_max_gb: u64,
}

impl TestConfig {
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestRunConfig {
    pub name: String,
    pub tool: String,
    pub config: String,
    pub max_parallel: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct InstanceConfig {
    pub num_instances: usize,
    pub seed: u64,
    pub petri_net_counters: usize,
    pub petri_net_transitions: usize,
    pub petri_net_max_tokens_per_transition: usize,
    pub petri_net_no_guards: bool,
    pub hand_picked_instances: Vec<String>,
    pub generate_vass: bool,
    pub vass_counters: InclusiveUsizeRange,
    pub vass_states: InclusiveUsizeRange,
    pub vass_transitions: InclusiveUsizeRange,
    pub vass_updates: InclusiveI32Range,
    pub vass_valuations: InclusiveI32Range,
    pub search_timeout_ms: u64,
    pub search_max_iterations: u64,
    pub search_repetitions: usize,
    pub minimize_instance: Option<String>,
}

impl Default for InstanceConfig {
    fn default() -> Self {
        Self {
            num_instances: 0,
            seed: 0,
            petri_net_counters: 0,
            petri_net_transitions: 0,
            petri_net_max_tokens_per_transition: 0,
            petri_net_no_guards: false,
            hand_picked_instances: vec![],
            generate_vass: false,
            vass_counters: InclusiveUsizeRange { min: 2, max: 4 },
            vass_states: InclusiveUsizeRange { min: 1, max: 4 },
            vass_transitions: InclusiveUsizeRange { min: 3, max: 10 },
            vass_updates: InclusiveI32Range { min: -3, max: 3 },
            vass_valuations: InclusiveI32Range { min: 0, max: 3 },
            search_timeout_ms: 2_000,
            search_max_iterations: 8,
            search_repetitions: 2,
            minimize_instance: None,
        }
    }
}

impl InstanceConfig {
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct InclusiveUsizeRange {
    pub min: usize,
    pub max: usize,
}

impl InclusiveUsizeRange {
    pub fn validate(&self, name: &str) -> anyhow::Result<()> {
        if self.min > self.max {
            anyhow::bail!("{name}.min must be <= {name}.max");
        }
        Ok(())
    }
}

impl Default for InclusiveUsizeRange {
    fn default() -> Self {
        Self { min: 0, max: 0 }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct InclusiveI32Range {
    pub min: i32,
    pub max: i32,
}

impl InclusiveI32Range {
    pub fn validate(&self, name: &str) -> anyhow::Result<()> {
        if self.min > self.max {
            anyhow::bail!("{name}.min must be <= {name}.max");
        }
        Ok(())
    }
}

impl Default for InclusiveI32Range {
    fn default() -> Self {
        Self { min: 0, max: 0 }
    }
}

pub type ToolConfig = HashMap<String, path::PathBuf>;

pub fn load_tool_config() -> anyhow::Result<ToolConfig> {
    let config_path = path::Path::new("./tools.config.toml");
    let canonic_path = fs::canonicalize(config_path)
        .with_context(|| format!("failed to canonicalize: {}", config_path.display()))?;
    let content = fs::read_to_string(&canonic_path)
        .with_context(|| format!("failed to read: {}", canonic_path.display()))?;
    let config: HashMap<String, String> = toml::from_str(&content)
        .with_context(|| format!("failed to parse: {}", canonic_path.display()))?;
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

pub fn load_ui_config() -> anyhow::Result<UIConfig> {
    let config_path = path::Path::new("./ui.config.toml");
    let canonic_path = fs::canonicalize(config_path)?;
    let content = fs::read_to_string(canonic_path)?;
    Ok(toml::from_str(&content)?)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolResult {
    pub tool: String,
    pub run_name: String,
    pub results: HashMap<String, SolverResultStatistic>,
}

impl ToolResult {
    pub fn new(
        tool: String,
        run_name: String,
        results: HashMap<String, SolverResultStatistic>,
    ) -> Self {
        Self {
            tool,
            run_name,
            results,
        }
    }
}
