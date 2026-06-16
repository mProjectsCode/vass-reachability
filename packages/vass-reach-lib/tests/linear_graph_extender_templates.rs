use vass_reach_lib::automaton::{
    InitializedAutomaton, ModifiableAutomaton,
    cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
    dfa::node::DfaNode,
    linear_graph::extender::template_testing::{
        analyze_template_bounds_snapshot, candidate_template_coefficients,
        default_template_coefficients, exact_successor_bound_from_coefficients,
        main_cfg_template_lower_bounds_snapshot, synthesize_template_coefficients,
    },
};

#[test]
fn lower_bounds_preserve_a_mandatory_increment() {
    let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(1));
    let initial = cfg.add_node(DfaNode::non_accepting(()));
    let accepting = cfg.add_node(DfaNode::accepting(()));
    cfg.set_initial(initial);
    cfg.add_edge(&initial, &accepting, CFGCounterUpdate::new(0, true));
    cfg.add_edge(&accepting, &initial, CFGCounterUpdate::new(0, false));

    let bounds = main_cfg_template_lower_bounds_snapshot(&cfg, &vec![0].into());

    assert_eq!(
        bounds.state_bounds[initial.index()].as_deref(),
        Some(&[0][..])
    );
    assert_eq!(
        bounds.state_bounds[accepting.index()].as_deref(),
        Some(&[1][..])
    );
}

#[test]
fn lower_bounds_are_weakened_by_decrement_cycles() {
    let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(1));
    let initial = cfg.add_node(DfaNode::accepting(()));
    cfg.set_initial(initial);
    cfg.add_edge(&initial, &initial, CFGCounterUpdate::new(0, false));

    let bounds = main_cfg_template_lower_bounds_snapshot(&cfg, &vec![100].into());

    assert_eq!(
        bounds.state_bounds[initial.index()].as_deref(),
        Some(&[0][..])
    );
}

#[test]
fn template_lower_bounds_preserve_guarded_nonzero_sum() {
    let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(2));
    let initial = cfg.add_node(DfaNode::accepting(()));
    let first_decrement = cfg.add_node(DfaNode::non_accepting(()));
    let second_decrement = cfg.add_node(DfaNode::non_accepting(()));
    let transfer = cfg.add_node(DfaNode::non_accepting(()));
    cfg.set_initial(initial);

    cfg.add_edge(&initial, &first_decrement, CFGCounterUpdate::new(0, true));
    cfg.add_edge(
        &first_decrement,
        &second_decrement,
        CFGCounterUpdate::new(1, false),
    );
    cfg.add_edge(&second_decrement, &initial, CFGCounterUpdate::new(1, false));
    cfg.add_edge(&initial, &transfer, CFGCounterUpdate::new(0, false));
    cfg.add_edge(&transfer, &initial, CFGCounterUpdate::new(1, true));

    let bounds = main_cfg_template_lower_bounds_snapshot(&cfg, &vec![1, 0].into());
    let sum_template = bounds
        .templates
        .iter()
        .position(|template| template.as_slice() == [1, 1])
        .unwrap();

    assert_eq!(
        bounds.state_bounds[initial.index()].as_ref().unwrap()[sum_template],
        1
    );
}

#[test]
fn exact_transfer_combines_relational_constraints() {
    let templates = default_template_coefficients(3);
    let source_bounds = vec![0, 0, 0, 2, 2, 2, 0];
    let all_counters = templates.len() - 1;
    let bound = exact_successor_bound_from_coefficients(
        &templates,
        &source_bounds,
        &CFGCounterUpdate::new(0, true),
        all_counters,
        10,
    );

    assert_eq!(bound, 4);
}

#[test]
fn candidate_generation_includes_weighted_templates() {
    let existing = default_template_coefficients(2);
    let candidates = candidate_template_coefficients(2, 2, 32, &existing);

    assert!(
        candidates
            .iter()
            .any(|template| template.as_slice() == [2, 1] || template.as_slice() == [1, 2])
    );
}

#[test]
fn weighted_template_proves_a_non_default_invariant() {
    let cfg = weighted_template_cfg();
    let initial = cfg.get_initial();

    let mut templates = default_template_coefficients(2);
    templates.push(vec![2, 1]);
    let analysis = analyze_template_bounds_snapshot(&cfg, &vec![1, 0].into(), &templates);
    let weighted = analysis.templates.len() - 1;

    assert_eq!(
        analysis.state_bounds[initial.index()].as_ref().unwrap()[weighted],
        2
    );
}

#[test]
fn synthesis_discovers_a_weighted_separating_template() {
    let cfg = weighted_template_cfg();
    let initial = cfg.get_initial();
    let initial_valuation = vec![1, 0].into();
    let template = synthesize_template_coefficients(
        &cfg,
        &initial_valuation,
        &[(initial, vec![0, 1].into())],
        2,
        32,
    )
    .unwrap();

    assert!(template.as_slice() == [2, 1] || template.as_slice() == [1, 2]);
}

fn weighted_template_cfg() -> VASSCFG<()> {
    let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(2));
    let initial = cfg.add_node(DfaNode::accepting(()));
    let dec_c0 = cfg.add_node(DfaNode::non_accepting(()));
    let first_inc_c1 = cfg.add_node(DfaNode::non_accepting(()));
    let inc_c0 = cfg.add_node(DfaNode::non_accepting(()));
    let first_dec_c1 = cfg.add_node(DfaNode::non_accepting(()));
    cfg.set_initial(initial);

    cfg.add_edge(&initial, &dec_c0, CFGCounterUpdate::new(0, false));
    cfg.add_edge(&dec_c0, &first_inc_c1, CFGCounterUpdate::new(1, true));
    cfg.add_edge(&first_inc_c1, &initial, CFGCounterUpdate::new(1, true));
    cfg.add_edge(&initial, &inc_c0, CFGCounterUpdate::new(0, true));
    cfg.add_edge(&inc_c0, &first_dec_c1, CFGCounterUpdate::new(1, false));
    cfg.add_edge(&first_dec_c1, &initial, CFGCounterUpdate::new(1, false));

    cfg
}
