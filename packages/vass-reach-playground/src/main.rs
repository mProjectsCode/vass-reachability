#![allow(dead_code, unused_imports)]

use std::time::Duration;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use vass_reach_lib::{
    automaton::{
        AutomatonIterators, InitializedAutomaton, ModifiableAutomaton,
        algorithms::EdgeAutomatonAlgorithms,
        linear_graph::{LinearGraph, part::LinearGraphRegion},
        petri_net::{PetriNet, initialized::InitializedPetriNet, spec::PetriNetSpec},
        scc::SCCAlgorithms,
        vass::{VASS, VASSEdge, initialized::InitializedVASS},
    },
    config::{PreprocessingConfig, VASSReachConfig},
    solver::{SolverStatus, vass_reach::VASSReachSolver},
};

mod minimization;


fn main() {
    let filter = tracing_subscriber::filter::Targets::new()
        .with_default(tracing::Level::INFO)
        .with_target("z3", tracing::Level::INFO);

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    // let lim_cfg = build_bounded_counting_cfg(1,
    // CFGCounterUpdate::new(1).unwrap(), 4, 0, 0); let rev_lim_cfg =
    // build_rev_bounded_counting_cfg(1, CFGCounterUpdate::new(1).unwrap(), 4, 0,
    // 0);

    // println!("Limit CFG: {:#?}", &lim_cfg);
    // println!("{}", lim_cfg.to_graphviz(None as Option<Path>));

    // println!("Reverse Limit CFG: {:#?}", &rev_lim_cfg);
    // println!("{}", rev_lim_cfg.to_graphviz(None as Option<Path>));

    // let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    // let q0 = vass.add_state(0);
    // let q1 = vass.add_state(1);

    // vass.add_transition(q0, q0, ('a', vec![1, 0].into()));
    // vass.add_transition(q0, q1, ('b', vec![-2, 0].into()));
    // vass.add_transition(q1, q1, ('b', vec![-1, 0].into()));

    // let initialized_vass = vass.init(vec![0, 0].into(), vec![0, 0].into(), q0,
    // q1);

    // let logger = Logger::new(
    //     vass_reach_lib::logger::LogLevel::Debug,
    //     "".to_string(),
    //     None,
    // );

    // let res = VASSReachSolver::new(
    //     &initialized_vass,
    //     // some time that is long enough, but makes the test run in a reasonable
    // time     VASSReachConfig::default().
    // with_timeout(Some(Duration::from_secs(5))),     Some(&logger),
    // )
    // .solve();

    // det();
    // lim_cfg_test();

    // difficult_instance();
    // other_instance();
    new_difficult_instances();
}

fn difficult_instance() {
    //                                              a1 + 6 >= a0 & a1 >= 0 & a0 >= 0
    // (a0)^6      .     (a0a1 cup a0'a1' cup a0)*                 .
    // (a0a1a1 cup a0'a1'a1')*    .         (a1 cup a1')*
    let mut vass = VASS::new(2, (0..10).collect());

    let s0 = vass.add_node(());
    let s1 = vass.add_node(());
    let s2 = vass.add_node(());
    let s3 = vass.add_node(());

    let _e0 = vass.add_edge(&s0, &s1, VASSEdge::new(0, vec![6, 0].into()));

    let _e1 = vass.add_edge(&s1, &s1, VASSEdge::new(1, vec![1, 1].into()));
    let _e2 = vass.add_edge(&s1, &s1, VASSEdge::new(2, vec![-1, -1].into()));
    let _e3 = vass.add_edge(&s1, &s1, VASSEdge::new(3, vec![1, 0].into()));

    let _e4 = vass.add_edge(&s1, &s2, VASSEdge::new(4, vec![0, 0].into()));

    let _e5 = vass.add_edge(&s2, &s2, VASSEdge::new(5, vec![1, 2].into()));
    let _e6 = vass.add_edge(&s2, &s2, VASSEdge::new(6, vec![-1, -2].into()));

    let _e7 = vass.add_edge(&s2, &s3, VASSEdge::new(7, vec![0, 0].into()));

    let _e8 = vass.add_edge(&s3, &s3, VASSEdge::new(8, vec![0, 1].into()));
    let _e9 = vass.add_edge(&s3, &s3, VASSEdge::new(9, vec![0, -1].into()));

    let initialized = vass.init(vec![0, 0].into(), vec![0, 0].into(), s0, s3);

    let cfg = initialized.to_cfg();
    let dag = cfg.find_scc_dag();
    println!("{}", dag.to_graphviz(None, None, true));
    println!("{}", cfg.to_graphviz(None, None));

    let _res = VASSReachSolver::new(
        &initialized,
        // some time that is long enough, but makes the test run in a reasonable time
        VASSReachConfig::default()
            .with_timeout(Some(Duration::from_mins(5)))
            .with_max_iterations(Some(100))
            .with_bounded_counting_enabled(false)
            .with_preprocessing(PreprocessingConfig::default().with_enabled(false)),
    )
    .solve();
}

fn other_instance() {
    let spec_str = "vars
    p1 p2 p3
rules
    p3 >= 1 ->
        p1' = p1+2,
        p2' = p2+2,
        p3' = p3-1;
    p1 >= 2, p2 >= 2 ->
        p1' = p1-2,
        p2' = p2-2;
    p1 >= 0 ->
        p1' = p1+1,
        p2' = p2+2,
        p3' = p3+1;
init
    p1=2, p2=0, p3=1
target
    p1=0, p2=0, p3=0
";

    // Reachable: two times t3, to get 4 4 3, then 3 times t1 to get 10 10 0, then
    // t2 5 times to get 0 0 0

    let spec = PetriNetSpec::parse(spec_str).unwrap();
    let petri_net = InitializedPetriNet::try_from(spec).unwrap();
    let initialized = petri_net.to_vass();

    let _res = VASSReachSolver::new(
        &initialized,
        // some time that is long enough, but makes the test run in a reasonable time
        VASSReachConfig::default()
            .with_timeout(Some(Duration::from_mins(5)))
            .with_max_iterations(Some(100))
            .with_bounded_counting_enabled(false)
            .with_preprocessing(PreprocessingConfig::default().with_enabled(false)),
    )
    .solve();
}

fn new_difficult_instances() {
    let instances = [
        ("new_difficult_instance_1", new_difficult_instance_1()),
        ("new_difficult_instance_2", new_difficult_instance_2()),
        ("new_difficult_instance_3", new_difficult_instance_3()),
    ];

    let results = instances
        .into_iter()
        .map(|(name, instance)| solve_difficult(name, instance))
        .collect::<Vec<_>>();

    println!("\nSolver overview");
    println!(
        "{:<28} {:>3} {:>6} {:>11} {:<32} {:>5} {:>10}",
        "instance", "dim", "states", "transitions", "status", "steps", "time"
    );
    for result in results {
        println!(
            "{:<28} {:>3} {:>6} {:>11} {:<32} {:>5} {:>9.3}s",
            result.name,
            result.dimension,
            result.states,
            result.transitions,
            result.status,
            result.steps,
            result.elapsed.as_secs_f64()
        );
    }
}

fn new_difficult_instance_1() -> InitializedVASS<(), usize> {
    let mut vass = VASS::new(2, (0..3).collect());
    let q = vass.add_node(());

    vass.add_edge(&q, &q, VASSEdge::new(0, vec![1, -2].into()));
    vass.add_edge(&q, &q, VASSEdge::new(1, vec![1, 0].into()));
    vass.add_edge(&q, &q, VASSEdge::new(2, vec![-1, 1].into()));

    vass.init(vec![1, 0].into(), vec![0, 0].into(), q, q)
}

fn new_difficult_instance_2() -> InitializedVASS<(), usize> {
    let mut vass = VASS::new(2, (0..3).collect());
    let q = vass.add_node(());

    vass.add_edge(&q, &q, VASSEdge::new(0, vec![-1, 1].into()));
    vass.add_edge(&q, &q, VASSEdge::new(1, vec![0, 1].into()));
    vass.add_edge(&q, &q, VASSEdge::new(2, vec![1, -2].into()));

    vass.init(vec![0, 1].into(), vec![0, 0].into(), q, q)
}

fn new_difficult_instance_3() -> InitializedVASS<(), usize> {
    let mut vass = VASS::new(1, (0..3).collect());
    let q0 = vass.add_node(());
    let q1 = vass.add_node(());

    vass.add_edge(&q0, &q1, VASSEdge::new(0, vec![1].into()));
    vass.add_edge(&q1, &q0, VASSEdge::new(1, vec![0].into()));
    vass.add_edge(&q0, &q0, VASSEdge::new(2, vec![-1].into()));

    vass.init(vec![0].into(), vec![0].into(), q0, q1)
}

struct DifficultInstanceResult {
    name: &'static str,
    dimension: usize,
    states: usize,
    transitions: usize,
    status: String,
    steps: u64,
    elapsed: Duration,
}

fn solve_difficult(
    name: &'static str,
    initialized: InitializedVASS<(), usize>,
) -> DifficultInstanceResult {
    tracing::info!(name, "solving difficult instance");
    let dimension = initialized.dimension();
    let states = initialized.state_count();
    let transitions = initialized.transition_count();

    print_rooted_linear_decomposition(name, &initialized);

    let result = VASSReachSolver::new(
        &initialized,
        VASSReachConfig::default()
            .with_timeout(Some(Duration::from_secs(30)))
            .with_max_iterations(Some(1))
            .with_bounded_counting_enabled(false)
            .with_preprocessing(PreprocessingConfig::default().with_enabled(false)),
    )
    .solve();

    tracing::info!(
        name,
        status = ?result.status,
        steps = result.statistics.step_count,
        elapsed = ?result.statistics.time,
        "solver finished"
    );

    DifficultInstanceResult {
        name,
        dimension,
        states,
        transitions,
        status: format!("{:?}", result.status),
        steps: result.statistics.step_count,
        elapsed: result.statistics.time,
    }
}

fn print_rooted_linear_decomposition(name: &str, initialized: &InitializedVASS<(), usize>) {
    let mut cfg = initialized.to_cfg();
    cfg.remove_trapping_states();

    println!("\n// {name}: source CFG");
    println!("{}", cfg.to_graphviz(None, None));

    let nodes = cfg.iter_node_indices().collect::<Vec<_>>();
    let accepting = nodes
        .iter()
        .copied()
        .filter(|node| cfg.is_accepting(node))
        .collect::<Vec<_>>();

    for (accepting_index, final_state) in accepting.into_iter().enumerate() {
        let region = LinearGraphRegion::from_subset(&cfg, &nodes, cfg.get_initial(), final_state);
        let mut linear_graph = LinearGraph::empty(&cfg, initialized.dimension());
        linear_graph.add_graph(region);

        match linear_graph.refine_to_rooted() {
            Ok(rooted_graphs) if rooted_graphs.is_empty() => {
                println!(
                    "// {name}: accepting CFG state {} has an empty rooted decomposition",
                    final_state.index()
                );
            }
            Ok(rooted_graphs) => {
                for (decomposition_index, rooted) in rooted_graphs.iter().enumerate() {
                    println!(
                        "\n// {name}: rooted decomposition {}.{} for accepting CFG state {}",
                        accepting_index + 1,
                        decomposition_index + 1,
                        final_state.index()
                    );
                    println!("{}", rooted.to_graphviz());
                }
            }
            Err(error) => {
                println!(
                    "// {name}: failed to build rooted decomposition for accepting CFG state {}: {error}",
                    final_state.index()
                );
            }
        }
    }
}
