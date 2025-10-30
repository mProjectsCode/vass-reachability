use hashbrown::HashMap;
use itertools::Itertools;
use petgraph::graph::EdgeIndex;
use z3::{
    Model,
    ast::{Bool, Int},
};

use crate::automaton::{cfg::CFG, path::parikh_image::ParikhImage};

pub fn parikh_image_from_edge_map<'a>(
    edge_map: &HashMap<EdgeIndex, Int<'a>>,
    model: &Model<'a>,
) -> ParikhImage {
    ParikhImage::new(
        edge_map
            .iter()
            .map(|(id, var)| {
                (
                    *id,
                    model.get_const_interp(var).unwrap().as_u64().unwrap() as u32,
                )
            })
            .filter(|(_, count)| *count > 0)
            .collect(),
    )
}

pub fn forbid_parikh_image<'a>(
    parikh_image: &ParikhImage,
    cfg: &impl CFG,
    edge_map: &HashMap<EdgeIndex, Int<'a>>,
    solver: &z3::Solver<'a>,
    ctx: &'a z3::Context,
) {
    // bools that represent whether each individual edge in the component is
    // taken
    let edges = parikh_image
        .iter_edges()
        .map(|edge| edge_map.get(&edge).unwrap().ge(&Int::from_i64(ctx, 1)))
        .collect_vec();
    let edges_ref = edges.iter().collect_vec();

    // bool that represent whether each individual edge that is incoming from
    // the component is taken
    let incoming = parikh_image
        .get_incoming_edges(cfg)
        .iter()
        .map(|edge| edge_map.get(edge).unwrap().ge(&Int::from_i64(ctx, 1)))
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
