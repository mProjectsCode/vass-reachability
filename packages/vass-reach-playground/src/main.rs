use std::time::Duration;

use vass_reach_lib::{
    automaton::{
        ModifiableAutomaton,
        petri_net::spec::ToSpecFormat,
        vass::{VASS, VASSEdge},
    },
    config::VASSReachConfig,
    logger::Logger,
    solver::vass_reach::VASSReachSolver,
};

fn main() {
    // let lim_cfg = build_bounded_counting_cfg(1, CFGCounterUpdate::new(1).unwrap(), 4, 0, 0);
    // let rev_lim_cfg = build_rev_bounded_counting_cfg(1, CFGCounterUpdate::new(1).unwrap(), 4, 0, 0);

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

    // let initialized_vass = vass.init(vec![0, 0].into(), vec![0, 0].into(), q0, q1);

    // let logger = Logger::new(
    //     vass_reach_lib::logger::LogLevel::Debug,
    //     "".to_string(),
    //     None,
    // );

    // let res = VASSReachSolver::new(
    //     &initialized_vass,
    //     // some time that is long enough, but makes the test run in a reasonable time
    //     VASSReachConfig::default().with_timeout(Some(Duration::from_secs(5))),
    //     Some(&logger),
    // )
    // .solve();

    // det();
    // lim_cfg_test();

    difficult_instance();
}

fn difficult_instance() {
    //                                              a1 + 6 >= a0 & a1 >= 0 & a0 >= 0
    // (a0)^6      .     (a0a1 cup a0'a1' cup a0)*                 .                 (a0a1a1 cup a0'a1'a1')*    .         (a1 cup a1')*
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

    let vas = initialized.to_vas();
    let petri_net = vas.to_petri_net();
    let spec = petri_net.to_spec_format();

    println!("{}", spec);

    let logger = Logger::new(
        vass_reach_lib::logger::LogLevel::Debug,
        "".to_string(),
        None,
    );

    let res = VASSReachSolver::new(
        &initialized,
        // some time that is long enough, but makes the test run in a reasonable time
        VASSReachConfig::default().with_timeout(Some(Duration::from_mins(5))),
        Some(&logger),
    )
    .solve();
}
