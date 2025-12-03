use std::time::Duration;

use vass_reach_lib::{
    automaton::{
        AutBuild, cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG}, dfa::{minimization::Minimizable, node::DfaNode}, nfa::NFA, path::Path, petri_net::{self, spec::ToSpecFormat}, vass::{VASS, counter::{VASSCounterIndex, VASSCounterUpdate}}
    },
    cfg_dec, cfg_inc,
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

fn lim_cfg_test() {
    let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(1));

    let q0 = cfg.add_state(DfaNode::accepting(()));
    let q1 = cfg.add_state(DfaNode::non_accepting(()));
    let q2 = cfg.add_state(DfaNode::non_accepting(()));

    cfg.set_start(q0);

    cfg.add_transition(q0, q0, cfg_inc!(0));
    cfg.add_transition(q0, q1, cfg_dec!(0));
    cfg.add_transition(q1, q2, cfg_dec!(0));
    cfg.add_transition(q2, q0, cfg_inc!(0));

    cfg.add_failure_state(());

    let initial_valuation = vec![1].into();
    let final_valuation = vec![0].into();

    let path = cfg
        .modulo_reach(4, &initial_valuation, &final_valuation)
        .unwrap();

    println!("Path: {:#?}", &path);

    let lim = path.to_bounded_counting_cfg(
        &cfg,
        &initial_valuation,
        &final_valuation,
        VASSCounterIndex::new(0),
    );

    println!("Limit CFG: {:#?}", &lim);
    println!("{}", lim.to_graphviz(None as Option<Path>));

    let rev_lim = path.to_rev_bounded_counting_cfg(
        &cfg,
        &initial_valuation,
        &final_valuation,
        VASSCounterIndex::new(0),
    );

    println!("Reverse Limit CFG: {:#?}", &rev_lim);
    println!("{}", rev_lim.to_graphviz(None as Option<Path>));

    let cut_cfg = cfg.intersect(&lim).intersect(&rev_lim);

    let min_cut_cfg = cut_cfg.minimize();

    println!("Cut CFG: {:#?}", &cut_cfg);
    println!("{}", cut_cfg.to_graphviz(None as Option<Path>));

    println!("Minimized Cut CFG: {:#?}", &min_cut_cfg);
    println!("{}", min_cut_cfg.to_graphviz(None as Option<Path>));

    // let overlap = lim.intersect(&rev_lim);

    // let min_overlap = overlap.minimize();

    // println!("Overlap CFG: {:#?}", &min_overlap);
    // println!("{}", min_overlap.to_graphviz(None as Option<Path>));
}

fn difficult_instance() {
    let mut vass = VASS::new(2, (0..10).collect());

    let s0 = vass.add_state(());
    let s1 = vass.add_state(());
    let s2 = vass.add_state(());
    let s3 = vass.add_state(());

    let _e0 = vass.add_transition(s0, s1, (0, vec![6, 0].into()));

    let _e1 = vass.add_transition(s1, s1, (1, vec![1, 1].into()));
    let _e2 = vass.add_transition(s1, s1, (2, vec![-1, -1].into()));
    let _e3 = vass.add_transition(s1, s1, (3, vec![1, 0].into()));

    let _e4 = vass.add_transition(s1, s2, (4, vec![0, 0].into()));

    let _e5 = vass.add_transition(s2, s2, (5, vec![1, 2].into()));
    let _e6 = vass.add_transition(s2, s2, (6, vec![-1, -2].into()));

    let _e7 = vass.add_transition(s2, s3, (7, vec![0, 0].into()));

    let _e8 = vass.add_transition(s3, s3, (8, vec![0, 1].into()));
    let _e9 = vass.add_transition(s3, s3, (9, vec![0, -1].into()));

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
