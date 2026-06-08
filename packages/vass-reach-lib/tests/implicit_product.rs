use hashbrown::HashSet;
use vass_reach_lib::{
    automaton::{
        Automaton, InitializedAutomaton, ModifiableAutomaton, TransitionSystem,
        cfg::{
            update::CFGCounterUpdate,
            vasscfg::{VASSCFG, build_bounded_counting_cfg, build_rev_bounded_counting_cfg},
        },
        dfa::node::DfaNode,
        implicit_cfg_product::{
            ImplicitCFGProduct, bounded_counting_indices, modulo_indices, state::MultiGraphState,
        },
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

    let implicit_product = ImplicitCFGProduct::new(
        1,
        initial_valuation.clone(),
        final_valuation.clone(),
        cfg,
        true,
    );

    let path = implicit_product.reach();

    assert!(
        inter
            .modulo_reach(3, &initial_valuation, &final_valuation)
            .is_none()
    );
    assert!(path.is_none());
}

#[test]
fn implicit_product_reach_paths_collects_multiple_accepting_paths() {
    let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(2));

    let q0 = cfg.add_node(DfaNode::non_accepting(()));
    let q1 = cfg.add_node(DfaNode::accepting(()));
    let q2 = cfg.add_node(DfaNode::accepting(()));

    cfg.set_initial(q0);
    cfg.add_edge(&q0, &q1, cfg_inc!(0));
    cfg.add_edge(&q0, &q2, cfg_inc!(1));

    let product =
        ImplicitCFGProduct::new_without_counting_cfgs(2, vec![0, 0].into(), vec![0, 0].into(), cfg);

    let paths = product.reach_paths(3);

    assert_eq!(paths.len(), 2);
    assert_eq!(
        paths
            .iter()
            .map(|path| path.transitions.as_slice())
            .collect::<Vec<_>>(),
        vec![&[cfg_inc!(0)][..], &[cfg_inc!(1)][..]]
    );
    assert_eq!(product.reach(), Some(paths[0].clone()));
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

#[test]
fn implicit_product_full_view_matches_product_transitions() {
    let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(1));

    let q0 = cfg.add_node(DfaNode::non_accepting(()));
    let q1 = cfg.add_node(DfaNode::accepting(()));

    cfg.set_initial(q0);
    cfg.add_edge(&q0, &q1, cfg_inc!(0));
    cfg.add_edge(&q1, &q1, cfg_inc!(0));
    cfg.add_edge(&q0, &q0, cfg_dec!(0));
    cfg.add_edge(&q1, &q0, cfg_dec!(0));
    cfg.set_complete_unchecked();

    let product = ImplicitCFGProduct::new_without_counting_cfgs(
        1,
        VASSCounterValuation::from(vec![0]),
        VASSCounterValuation::from(vec![1]),
        cfg,
    );
    let view = product.full_view();
    let initial = product.initial();
    let view_initial = view.get_initial();

    assert_eq!(view.active_cfg_indices(), &[0]);
    assert_eq!(view_initial, initial);
    assert_eq!(
        view.successor(&view_initial, &cfg_inc!(0)),
        product.successor(&initial, &cfg_inc!(0))
    );
    assert!(view.is_accepting(&view.successor(&view_initial, &cfg_inc!(0)).unwrap()));
}

#[test]
fn implicit_product_subset_view_uses_only_active_cfgs() {
    let mut a = VASSCFG::new(CFGCounterUpdate::alphabet(1));
    let a0 = a.add_node(DfaNode::new(false, false, ()));
    let a1 = a.add_node(DfaNode::new(false, false, ()));
    a.add_edge(&a0, &a1, cfg_inc!(0));
    a.add_edge(&a1, &a1, cfg_inc!(0));
    a.add_edge(&a0, &a0, cfg_dec!(0));
    a.add_edge(&a1, &a0, cfg_dec!(0));
    a.set_initial(a0);
    a.set_complete_unchecked();

    let mut b = VASSCFG::new(CFGCounterUpdate::alphabet(1));
    let b0 = b.add_node(DfaNode::new(false, false, ()));
    let b1 = b.add_node(DfaNode::new(false, false, ()));
    b.add_edge(&b0, &b0, cfg_inc!(0));
    b.add_edge(&b1, &b0, cfg_inc!(0));
    b.add_edge(&b0, &b1, cfg_dec!(0));
    b.add_edge(&b1, &b1, cfg_dec!(0));
    b.set_initial(b0);
    b.set_complete_unchecked();

    let mut product = ImplicitCFGProduct::new_without_counting_cfgs(
        1,
        VASSCounterValuation::from(vec![0]),
        VASSCounterValuation::from(vec![0]),
        a,
    );
    product.add_cfg(b);

    let view = product.view_from_indices([1]);
    let initial = view.get_initial();
    let successor = view.successor(&initial, &cfg_dec!(0)).unwrap();
    let predecessors = view
        .predecessors(&MultiGraphState::from(vec![b0]))
        .collect::<HashSet<_>>();

    assert_eq!(view.active_cfg_indices(), &[1]);
    assert_eq!(initial, MultiGraphState::from(vec![b0]));
    assert_eq!(successor, MultiGraphState::from(vec![b1]));
    assert_eq!(
        predecessors,
        HashSet::from_iter([
            MultiGraphState::from(vec![b0]),
            MultiGraphState::from(vec![b1])
        ])
    );
}

#[test]
fn implicit_product_view_projects_paths_to_compact_states() {
    let mut a = VASSCFG::new(CFGCounterUpdate::alphabet(1));
    let a0 = a.add_node(DfaNode::new(false, false, ()));
    let a1 = a.add_node(DfaNode::new(false, false, ()));
    a.add_edge(&a0, &a1, cfg_inc!(0));
    a.add_edge(&a1, &a1, cfg_inc!(0));
    a.add_edge(&a0, &a0, cfg_dec!(0));
    a.add_edge(&a1, &a0, cfg_dec!(0));
    a.set_initial(a0);
    a.set_complete_unchecked();

    let mut b = VASSCFG::new(CFGCounterUpdate::alphabet(1));
    let b0 = b.add_node(DfaNode::new(false, false, ()));
    let b1 = b.add_node(DfaNode::new(false, false, ()));
    b.add_edge(&b0, &b1, cfg_inc!(0));
    b.add_edge(&b1, &b1, cfg_inc!(0));
    b.add_edge(&b0, &b0, cfg_dec!(0));
    b.add_edge(&b1, &b0, cfg_dec!(0));
    b.set_initial(b0);
    b.set_complete_unchecked();

    let mut product = ImplicitCFGProduct::new_without_counting_cfgs(
        1,
        VASSCounterValuation::from(vec![0]),
        VASSCounterValuation::from(vec![0]),
        a,
    );
    product.add_cfg(b);

    let full_path = vass_reach_lib::automaton::path::Path::from_word(
        product.initial(),
        &[cfg_inc!(0)],
        &product,
    )
    .unwrap();
    let view = product.view_from_indices([1]);
    let projected = view.project_path(&full_path);

    assert_eq!(projected.transitions, full_path.transitions);
    assert_eq!(projected.start(), &MultiGraphState::from(vec![b0]));
    assert_eq!(projected.end(), &MultiGraphState::from(vec![b1]));
}

#[test]
fn implicit_product_view_without_modulo_cfgs_excludes_modulo_indices() {
    let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(2));
    let q0 = cfg.add_node(DfaNode::accepting(()));
    cfg.set_initial(q0);
    cfg.add_edge(&q0, &q0, cfg_inc!(0));
    cfg.add_edge(&q0, &q0, cfg_dec!(0));
    cfg.add_edge(&q0, &q0, cfg_inc!(1));
    cfg.add_edge(&q0, &q0, cfg_dec!(1));
    cfg.set_complete_unchecked();

    let product = ImplicitCFGProduct::new(
        2,
        VASSCounterValuation::from(vec![0, 0]),
        VASSCounterValuation::from(vec![0, 0]),
        cfg,
        false,
    );
    let view = product.view_without_modulo_cfgs();

    assert!(
        !view
            .active_cfg_indices()
            .iter()
            .any(|index| modulo_indices(2).contains(index))
    );
    assert_eq!(view.active_cfg_indices(), &[0, 3, 4, 5, 6]);
}

#[test]
fn implicit_product_can_disable_bounded_counting() {
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

    let implicit_product =
        ImplicitCFGProduct::new(1, initial_valuation, final_valuation, cfg, false);

    assert!(implicit_product.reach().is_some());

    for index in bounded_counting_indices(1) {
        let counting_cfg = &implicit_product.cfgs[index];

        assert_eq!(counting_cfg.node_count(), 1);
        assert!(counting_cfg.is_accepting(&counting_cfg.get_initial()));
    }
}
