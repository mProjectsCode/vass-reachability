use std::time::Duration;

use vass_reachability::{
    automaton::{
        AutBuild, AutomatonEdge, AutomatonNode,
        dfa::{
            DFA,
            cfg::{CFGCounterUpdate, build_rev_limited_counting_cfg},
            node::DfaNode,
        },
        path::Path,
        vass::{VASS, initialized::InitializedVASS},
    },
    boxed_slice,
    logger::Logger,
    solver::{vass_reach::VASSReachSolverOptions, vass_z_reach::VASSZReachSolverOptions},
};
// Seems like there are currently two types of issues:
// 1. longer and longer negative paths are found, see net 25. We probably need
//    to cut away some part of the loop.
// 2. Paths repeatedly overflowing mu. Seems like increasing mu to get rid of
//    simple paths is not always the solution.

fn main() {
    // let initialized_vass =
    //     InitializedPetriNet::from_file("test_data/petri_nets/4/unknown_1.json").
    // to_vass();

    // dbg!(&initialized_vass);
    // println!("{}", initialized_vass.to_cfg().to_graphviz());

    // let mut petri_net = PetriNet::new(4);

    // petri_net.add_transition(vec![(1, 1)], vec![(1, 2)]);
    // petri_net.add_transition(vec![(1, 2)], vec![(1, 3), (1, 4)]);
    // petri_net.add_transition(vec![(1, 3)], vec![(1, 2)]);

    // let initialized_petri_net = petri_net.init(boxed_slice![1, 0, 0, 0],
    // boxed_slice![0, 1, 0, 3]);

    // let initialized_vass = initialized_petri_net.to_vass();

    // let res = VASSReachSolverOptions::default()
    //     .with_iteration_limit(100)
    //     .with_time_limit(Duration::from_secs(20))
    //     .with_log_level(vass_reachability::logger::LogLevel::Debug)
    //     .with_log_file("logs/log.txt")
    //     .to_solver(initialized_vass)
    //     .solve();
    // dbg!(&res);

    // print_vass(&example());

    example5();

    // assert!(!res.unknown());
}

fn example() -> InitializedVASS<u32, u32> {
    let mut vass = VASS::<u32, u32>::new(2, vec![1, 2, 3, 4, 5]);

    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);
    let q2 = vass.add_state(2);

    vass.add_transition(q0, q0, (1, boxed_slice![2, 0]));
    vass.add_transition(q0, q1, (2, boxed_slice![-3, 1]));
    vass.add_transition(q0, q2, (3, boxed_slice![-3, 0]));
    vass.add_transition(q1, q1, (3, boxed_slice![2, 0]));
    vass.add_transition(q1, q2, (3, boxed_slice![-1, -1]));

    vass.init(boxed_slice!(0, 0), boxed_slice!(0, 0), q0, q2)
}

fn print_vass<N: AutomatonNode, E: AutomatonEdge>(vass: &InitializedVASS<N, E>) {
    let mut cfg = vass.to_cfg();
    println!("{}", cfg.to_graphviz(None as Option<Path>));
    cfg.add_failure_state(());
    cfg = cfg.minimize();
    cfg.remove_trapping_states();
    println!("{}", cfg.to_graphviz(None as Option<Path>));

    let path = cfg.modulo_reach(2, &[0, 0], &[0, 0]);
    println!("{}", cfg.to_graphviz(path));
}

fn example2() {
    let mut cfg1 = DFA::<(), CFGCounterUpdate>::new(CFGCounterUpdate::alphabet(1));
    let s0 = cfg1.add_state(DfaNode::new(false, ()));
    let s1 = cfg1.add_state(DfaNode::new(false, ()));
    let s2 = cfg1.add_state(DfaNode::new(true, ()));

    cfg1.add_transition(s0, s1, CFGCounterUpdate::new(1).unwrap());
    cfg1.add_transition(s1, s0, CFGCounterUpdate::new(1).unwrap());
    cfg1.add_transition(s0, s2, CFGCounterUpdate::new(-1).unwrap());

    cfg1.set_start(s0);

    cfg1.add_failure_state(());

    let mut mod_cfg = DFA::<(), CFGCounterUpdate>::new(CFGCounterUpdate::alphabet(1));
    let s0 = mod_cfg.add_state(DfaNode::new(true, ()));
    let s1 = mod_cfg.add_state(DfaNode::new(false, ()));

    mod_cfg.add_transition(s0, s1, CFGCounterUpdate::new(1).unwrap());
    mod_cfg.add_transition(s1, s0, CFGCounterUpdate::new(1).unwrap());
    mod_cfg.add_transition(s0, s1, CFGCounterUpdate::new(-1).unwrap());
    mod_cfg.add_transition(s1, s0, CFGCounterUpdate::new(-1).unwrap());

    mod_cfg.set_start(s0);

    mod_cfg.override_complete();

    let product = cfg1.intersect(&mod_cfg);

    println!("{}", product.to_graphviz(None as Option<Path>));
}

fn example3() {
    let mut vass = VASS::<(), usize>::new(2, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);

    let q0 = vass.add_state(());
    let q1 = vass.add_state(());
    let q2 = vass.add_state(());
    let q3 = vass.add_state(());

    vass.add_transition(q0, q1, (0, boxed_slice!(6, 0)));
    vass.add_transition(q1, q1, (1, boxed_slice!(1, 1)));
    vass.add_transition(q1, q1, (2, boxed_slice!(-1, -1)));
    vass.add_transition(q1, q1, (3, boxed_slice!(1, 0)));
    vass.add_transition(q1, q2, (4, boxed_slice!(0, 0)));
    vass.add_transition(q2, q2, (5, boxed_slice!(1, 2)));
    vass.add_transition(q2, q2, (6, boxed_slice!(-1, -2)));
    vass.add_transition(q2, q3, (7, boxed_slice!(0, 0)));
    vass.add_transition(q3, q3, (8, boxed_slice!(0, 1)));
    vass.add_transition(q3, q3, (9, boxed_slice!(0, -1)));

    let init = vass.init(boxed_slice!(0, 0), boxed_slice!(0, 0), q0, q3);

    let res = VASSReachSolverOptions::default()
        .with_iteration_limit(10000)
        .with_time_limit(Duration::from_secs(60))
        .with_log_level(vass_reachability::logger::LogLevel::Debug)
        .with_log_file("logs/log.txt")
        .to_solver(init)
        .solve();

    dbg!(&res);
}

fn example4() {
    let mut vass = VASS::<(), usize>::new(2, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);

    let q1 = vass.add_state(());
    let q2 = vass.add_state(());

    vass.add_transition(q1, q1, (1, boxed_slice!(1, 1)));
    vass.add_transition(q1, q1, (2, boxed_slice!(-1, -1)));
    vass.add_transition(q1, q1, (3, boxed_slice!(1, 0)));
    vass.add_transition(q1, q2, (4, boxed_slice!(0, 0)));
    vass.add_transition(q2, q2, (8, boxed_slice!(0, 1)));
    vass.add_transition(q2, q2, (9, boxed_slice!(0, -1)));

    let init = vass.init(boxed_slice!(6, 0), boxed_slice!(0, 0), q1, q2);

    let mut cfg = init.to_cfg();
    // cfg.add_failure_state(());
    // cfg = cfg.minimize();

    println!("{}", cfg.to_graphviz(None as Option<Path>));

    let res = VASSReachSolverOptions::default()
        .with_iteration_limit(10000)
        .with_time_limit(Duration::from_secs(60))
        .with_log_level(vass_reachability::logger::LogLevel::Debug)
        .with_log_file("logs/log.txt")
        .to_solver(init)
        .solve();

    // let logger = Logger::new(
    //     vass_reachability::logger::LogLevel::Debug,
    //     "z_reach".to_string(),
    //     None,
    // );
    // let res = VASSZReachSolverOptions::default()
    //     .with_iteration_limit(10)
    //     .with_logger(logger)
    //     .to_solver(cfg, boxed_slice!(6, 0), boxed_slice!(0, 0))
    //     .solve();

    dbg!(&res);
}

fn example5() {
    let mut count = build_rev_limited_counting_cfg(1, CFGCounterUpdate::new(1).unwrap(), 3, 2);
    count = count.minimize();

    println!("{}", count.to_graphviz(None as Option<Path>));
}
