use hashbrown::HashSet;
use vass_reach_lib::{
    automaton::{
        ModifiableAutomaton, TransitionSystem,
        cfg::{
            update::CFGCounterUpdate,
            vasscfg::{VASSCFG, build_bounded_counting_cfg, build_rev_bounded_counting_cfg},
        },
        dfa::node::DfaNode,
        implicit_cfg_product::{ImplicitCFGProduct, state::MultiGraphState},
        vass::counter::{VASSCounterIndex, VASSCounterValuation},
    },
    cfg_dec, cfg_inc,
};

#[test]
fn implicit_product_test() {
    let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(1));

    let q0 = cfg.add_node(DfaNode::accepting(()));
    let q1 = cfg.add_node(DfaNode::non_accepting(()));
    let q2 = cfg.add_node(DfaNode::non_accepting(()));

    cfg.set_initial(q0);

    cfg.add_edge(&q0, &q0, cfg_inc!(0));
    cfg.add_edge(&q0, &q1, cfg_dec!(0));
    cfg.add_edge(&q1, &q2, cfg_dec!(0));
    cfg.add_edge(&q2, &q0, cfg_inc!(0));

    cfg.make_complete(());

    let initial_valuation = VASSCounterValuation::from(vec![1]);
    let final_valuation = VASSCounterValuation::from(vec![0]);

    let lim_cfg = build_bounded_counting_cfg(1, VASSCounterIndex::new(0), 3, 1, 0);
    let rev_lim_cfg = build_rev_bounded_counting_cfg(1, VASSCounterIndex::new(0), 3, 1, 0);

    lim_cfg.assert_complete();
    rev_lim_cfg.assert_complete();

    let inter = cfg.intersect(&lim_cfg).intersect(&rev_lim_cfg);

    let implicit_product =
        ImplicitCFGProduct::new(1, initial_valuation.clone(), final_valuation.clone(), cfg);

    let path = implicit_product.reach();

    assert!(
        inter
            .modulo_reach(3, &initial_valuation, &final_valuation)
            .is_none()
    );
    assert!(path.is_none());
}

#[test]
fn implicit_product_predecessors() {
    // dimension = 1 => alphabet {+c0, -c0}
    let mut a = VASSCFG::new(CFGCounterUpdate::alphabet(1));
    let a0 = a.add_node(DfaNode::new(false, false, ()));
    let a1 = a.add_node(DfaNode::new(false, false, ()));
    // +c0: a0 -> a1, a1 -> a1
    a.add_edge(&a0, &a1, cfg_inc!(0));
    a.add_edge(&a1, &a1, cfg_inc!(0));
    // -c0: no edge into a1 (targets go to a0)
    a.add_edge(&a0, &a0, cfg_dec!(0));
    a.add_edge(&a1, &a0, cfg_dec!(0));
    a.set_initial(a0);
    a.set_complete_unchecked();

    let mut b = VASSCFG::new(CFGCounterUpdate::alphabet(1));
    let b0 = b.add_node(DfaNode::new(false, false, ()));
    let b1 = b.add_node(DfaNode::new(false, false, ()));
    // +c0: b0 -> b0, b1 -> b0  (predecessors of b0 under +c0 are {b0,b1})
    b.add_edge(&b0, &b0, cfg_inc!(0));
    b.add_edge(&b1, &b0, cfg_inc!(0));
    // -c0: edges do not target b0
    b.add_edge(&b0, &b1, cfg_dec!(0));
    b.add_edge(&b1, &b1, cfg_dec!(0));
    b.set_initial(b0);
    b.set_complete_unchecked();

    let mut prod = ImplicitCFGProduct::new_without_counting_cfgs(
        1,
        VASSCounterValuation::from(vec![0]),
        VASSCounterValuation::from(vec![0]),
        a,
    );
    prod.add_cfg(b);

    let target = MultiGraphState::from(vec![a1, b0]);
    let predecessors: HashSet<_> = prod.predecessors(&target).collect();

    let mut expected = HashSet::new();
    expected.insert(MultiGraphState::from(vec![a0, b0]));
    expected.insert(MultiGraphState::from(vec![a0, b1]));
    expected.insert(MultiGraphState::from(vec![a1, b0]));
    expected.insert(MultiGraphState::from(vec![a1, b1]));

    assert_eq!(predecessors, expected);
}
