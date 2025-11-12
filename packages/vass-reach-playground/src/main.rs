use std::time::Duration;

use vass_reach_lib::{
    automaton::{
        AutBuild, cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG}, dfa::{minimization::Minimizable, node::DfaNode}, nfa::NFA, path::Path, vass::{VASS, counter::VASSCounterIndex}
    },
    cfg_dec, cfg_inc,
    logger::Logger,
    solver::vass_reach::VASSReachSolverOptions,
};

fn main() {
    // let lim_cfg = build_bounded_counting_cfg(1, CFGCounterUpdate::new(1).unwrap(), 4, 0, 0);
    // let rev_lim_cfg = build_rev_bounded_counting_cfg(1, CFGCounterUpdate::new(1).unwrap(), 4, 0, 0);

    // println!("Limit CFG: {:#?}", &lim_cfg);
    // println!("{}", lim_cfg.to_graphviz(None as Option<Path>));

    // println!("Reverse Limit CFG: {:#?}", &rev_lim_cfg);
    // println!("{}", rev_lim_cfg.to_graphviz(None as Option<Path>));

    let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);

    vass.add_transition(q0, q0, ('a', vec![1, 0].into()));
    vass.add_transition(q0, q1, ('b', vec![-2, 0].into()));
    vass.add_transition(q1, q1, ('b', vec![-1, 0].into()));

    let initialized_vass = vass.init(vec![0, 0].into(), vec![0, 0].into(), q0, q1);

    let logger = Logger::new(
        vass_reach_lib::logger::LogLevel::Debug,
        "".to_string(),
        None,
    );
    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
        .with_time_limit(Duration::from_secs(5)) // some time that is long enough, but makes the test run in a reasonable time
        .with_logger(&logger)
        .to_vass_solver(&initialized_vass)
        .solve();


    det();
    // lim_cfg_test();
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

fn det() {
    let mut nfa = NFA::<(), char>::new(vec!['a', 'b', 'c']);
    let q0 = nfa.graph.add_node(DfaNode::non_accepting(()));
    let q1 = nfa.graph.add_node(DfaNode::non_accepting(()));
    let q2 = nfa.graph.add_node(DfaNode::non_accepting(()));
    let q3 = nfa.graph.add_node(DfaNode::accepting(()));

    nfa.set_start(q0);

    nfa.add_transition(q0, q0, Some('a'));
    nfa.add_transition(q0, q1, Some('a'));
    nfa.add_transition(q1, q2, Some('b'));
    nfa.add_transition(q1, q3, Some('b'));
    nfa.add_transition(q2, q3, Some('c'));
    nfa.add_transition(q3, q1, Some('a'));

    let dfa = nfa.determinize();

    println!("{}", dfa.to_graphviz(None as Option<Path>));
}