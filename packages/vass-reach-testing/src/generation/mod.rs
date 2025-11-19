use vass_reach_lib::logger::Logger;

use crate::{Args, config::{CustomError, Test}, random::{RandomOptions, petri_net::generate_random_petri_net}};

pub fn generate(logger: &Logger, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let Some(folder) = &args.folder else {
        return CustomError::str("missing required folder argument").to_boxed()
    };
    let test = Test::canonicalize(&folder)?;
    let config = test.instance_config()?;

    logger.info("Generating random Petri nets...");

    let random_petri_nets = generate_random_petri_net(
        RandomOptions::new(config.seed, config.num_instances),
        config.petri_net_counters,
        config.petri_net_transitions,
        config.petri_net_max_tokens_per_transition,
        config.petri_net_no_guards
    );

    logger.info(&format!(
        "Generated {} random Petri nets.",
        random_petri_nets.len()
    ));

    test.write_nets(&random_petri_nets)?;

    logger.info(&format!(
        "Persisted random Petri nets to folder: {}",
        test.instances_folder().display()
    ));

    Ok(())
}