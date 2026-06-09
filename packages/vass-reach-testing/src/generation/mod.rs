use crate::{
    Args,
    config::Test,
    random::{
        RandomOptions, petri_net::generate_random_petri_net, vass::generate_random_vass_in_ranges,
    },
};

pub fn generate(args: &Args) -> anyhow::Result<()> {
    let Some(folder) = &args.folder else {
        anyhow::bail!("missing required folder argument");
    };
    let test = Test::canonicalize(folder)?;
    let config = test.instance_config()?;

    if config.num_instances == 0 {
        anyhow::bail!(
            "instance generation requested but num_instances is 0; set random generation parameters in instances.toml"
        );
    }

    if config.generate_vass {
        tracing::info!("Generating random VASS instances...");
        let instances = generate_random_vass_in_ranges(
            RandomOptions::new(config.seed, config.num_instances),
            config.vass_counters,
            config.vass_states,
            config.vass_transitions,
            config.vass_updates,
            config.vass_valuations,
        )?;
        test.write_vass_instances(&instances)?;
        tracing::info!("Generated {} random VASS instances.", instances.len());
        return Ok(());
    }

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
