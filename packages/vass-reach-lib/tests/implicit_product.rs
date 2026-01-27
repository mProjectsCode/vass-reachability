use vass_reach_lib::{
    automaton::{
        ModifiableAutomaton,
        cfg::{
            update::CFGCounterUpdate,
            vasscfg::{VASSCFG, build_bounded_counting_cfg, build_rev_bounded_counting_cfg},
        },
        dfa::node::DfaNode,
        implicit_cfg_product::ImplicitCFGProduct,
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

    cfg.add_edge(q0, q0, cfg_inc!(0));
    cfg.add_edge(q0, q1, cfg_dec!(0));
    cfg.add_edge(q1, q2, cfg_dec!(0));
    cfg.add_edge(q2, q0, cfg_inc!(0));

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
