use std::fs;

use serde::Serialize;
use vass_reach_lib::automaton::petri_net::initialized::InitializedPetriNet;

pub mod petri_net;
pub mod vass;

pub struct RandomOptions {
    pub seed: u64,
    pub count: usize,
}

impl Default for RandomOptions {
    fn default() -> Self {
        RandomOptions { seed: 1, count: 10 }
    }
}

impl RandomOptions {
    pub fn new(seed: u64, count: usize) -> Self {
        RandomOptions { seed, count }
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub fn with_count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }
}

fn persist_to_file<T: Serialize>(
    obj: &T,
    path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = fs::File::create(path)?;
    serde_json::to_writer_pretty(file, obj)?;
    Ok(())
}

pub fn persist_multiple_to_file<T: Serialize>(
    objs: &Vec<T>,
    folder_path: &std::path::Path,
    base_file_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if !folder_path.exists() {
        fs::create_dir_all(folder_path)?;
    }

    for (i, obj) in objs.iter().enumerate() {
        let file_path = folder_path.join(format!("{base_file_name}_{i}.json"));
        persist_to_file(obj, &file_path)?;
    }

    Ok(())
}

pub fn persist_nets_to_file(
    nets: &Vec<InitializedPetriNet>,
    folder_path: &std::path::Path,
    base_file_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if !folder_path.exists() {
        fs::create_dir_all(folder_path)?;
    }

    for (i, obj) in nets.iter().enumerate() {
        let file_path = folder_path.join(format!("{base_file_name}_{i}.spec"));
        obj.to_spec_file(file_path.to_str().unwrap())?;
    }

    Ok(())
}
