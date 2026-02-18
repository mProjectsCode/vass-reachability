use crate::{
    Args,
    config::Test,
    random::{RandomOptions, petri_net::generate_random_petri_net},
};

pub fn generate(args: &Args) -> anyhow::Result<()> {
    let Some(folder) = &args.folder else {
        anyhow::bail!("missing required folder argument");
    };
    let test = Test::canonicalize(folder)?;
    let config = test.instance_config()?;

    tracing::info!("Generating random Petri nets...");

    let random_petri_nets = generate_random_petri_net(
        RandomOptions::new(config.seed, config.num_instances),
        config.petri_net_counters,
        config.petri_net_transitions,
        config.petri_net_max_tokens_per_transition,
        config.petri_net_no_guards,
    );

    tracing::info!("Generated {} random Petri nets.", random_petri_nets.len());

    test.write_nets(&random_petri_nets)?;

    tracing::info!(
        "Persisted random Petri nets to folder: {}",
        test.instances_folder().display()
    );

    Ok(())
}
