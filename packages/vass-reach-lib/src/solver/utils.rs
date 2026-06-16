use itertools::Itertools;
use z3::{
    Model,
    ast::{Bool, Int},
};

use crate::automaton::{
    CompactGIndex,
    cfg::{ExplicitEdgeCFG, update::CFGCounterUpdate},
    index_map::OptionIndexMap,
    path::parikh_image::ParikhImage,
    vass::counter::VASSCounterValuation,
};

pub fn parikh_image_from_edge_map<EIndex: CompactGIndex>(
    edge_map: &OptionIndexMap<EIndex, Int>,
    model: &Model,
) -> ParikhImage<EIndex> {
    ParikhImage::new(
        edge_map.map(|var| model.get_const_interp(var).unwrap().as_u64().unwrap() as u32),
    )
}

pub fn forbid_parikh_image<C: ExplicitEdgeCFG>(
    parikh_image: &ParikhImage<C::EIndex>,
    cfg: &C,
    edge_map: &OptionIndexMap<C::EIndex, Int>,
    solver: &z3::Solver,
) {
    // bools that represent whether each individual edge in the component is
    // taken
    let edges = parikh_image
        .iter_edges()
        .map(|edge| edge_map.get(edge).as_ref().unwrap().ge(Int::from_i64(1)))
        .collect_vec();
    let edges_ref = edges.iter().collect_vec();

    // bool that represent whether each individual edge that is incoming from
    // the component is taken
    let incoming = parikh_image
        .get_incoming_edges(cfg)
        .iter()
        .map(|edge| edge_map.get(*edge).as_ref().unwrap().ge(Int::from_i64(1)))
        .collect_vec();
    let incoming_ref = incoming.iter().collect_vec();

    let edges_ast = Bool::and(&edges_ref);
    let incoming_ast = Bool::or(&incoming_ref);

    // CONSTRAINT: if all edges in the component are taken, then at least one
    // incoming edge must be taken as well this is because we
    // need to enter the component.
    // outgoing edges don't work because we my leave the component via a final
    // state
    solver.assert(edges_ast.implies(incoming_ast));
}

pub fn assert_non_negative(solver: &z3::Solver, value: &Int) {
    solver.assert(value.ge(Int::from_i64(0)));
}

pub fn add_cfg_update_to_sums(sums: &mut [Int], multiplier: &Int, update: &CFGCounterUpdate) {
    let counter = update.counter().to_usize();
    sums[counter] = &sums[counter] + multiplier * update.op_i64();
}

pub fn assert_sums_match_valuation(
    solver: &z3::Solver,
    sums: &[Int],
    valuation: &VASSCounterValuation,
) {
    for (sum, target) in sums.iter().zip(valuation.iter()) {
        solver.assert(sum.eq(Int::from_i64(*target as i64)));
    }
}
