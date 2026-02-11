use itertools::Itertools;
use z3::{
    Model,
    ast::{Bool, Int},
};

use crate::automaton::{
    CompactGIndex, cfg::ExplicitEdgeCFG, index_map::OptionIndexMap, path::parikh_image::ParikhImage,
};

pub fn parikh_image_from_edge_map<'a, EIndex: CompactGIndex>(
    edge_map: &OptionIndexMap<EIndex, Int<'a>>,
    model: &Model<'a>,
) -> ParikhImage<EIndex> {
    ParikhImage::new(
        edge_map.map(|var| model.get_const_interp(var).unwrap().as_u64().unwrap() as u32),
    )
}

pub fn forbid_parikh_image<'a, C: ExplicitEdgeCFG>(
    parikh_image: &ParikhImage<C::EIndex>,
    cfg: &C,
    edge_map: &OptionIndexMap<C::EIndex, Int<'a>>,
    solver: &z3::Solver<'a>,
    ctx: &'a z3::Context,
) {
    // bools that represent whether each individual edge in the component is
    // taken
    let edges = parikh_image
        .iter_edges()
        .map(|edge| {
            edge_map
                .get(edge)
                .as_ref()
                .unwrap()
                .ge(&Int::from_i64(ctx, 1))
        })
        .collect_vec();
    let edges_ref = edges.iter().collect_vec();

    // bool that represent whether each individual edge that is incoming from
    // the component is taken
    let incoming = parikh_image
        .get_incoming_edges(cfg)
        .iter()
        .map(|edge| {
            edge_map
                .get(*edge)
                .as_ref()
                .unwrap()
                .ge(&Int::from_i64(ctx, 1))
        })
        .collect_vec();
    let incoming_ref = incoming.iter().collect_vec();

    let edges_ast = Bool::and(ctx, &edges_ref);
    let incoming_ast = Bool::or(ctx, &incoming_ref);

    // CONSTRAINT: if all edges in the component are taken, then at least one
    // incoming edge must be taken as well this is because we
    // need to enter the component.
    // outgoing edges don't work because we my leave the component via a final
    // state
    solver.assert(&edges_ast.implies(&incoming_ast));
}
