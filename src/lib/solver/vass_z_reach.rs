use hashbrown::HashMap;
use itertools::Itertools;
use petgraph::{
    graph::{EdgeIndex, NodeIndex},
    visit::EdgeRef,
};
use z3::{
    ast::{Ast, Bool, Int},
    Config, Context, Solver,
};

use crate::{
    automaton::{dfa::VASSCFG, parikh_image::ParikhImage, AutomatonNode},
    logger::Logger,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolverStatus<T, F> {
    True(T),
    False(F),
}

impl From<bool> for SolverStatus<(), ()> {
    fn from(b: bool) -> Self {
        if b {
            SolverStatus::True(())
        } else {
            SolverStatus::False(())
        }
    }
}

impl<T, F> SolverStatus<T, F> {
    pub fn is_success(&self) -> bool {
        match &self {
            SolverStatus::True(_) => true,
            SolverStatus::False(_) => false,
        }
    }

    pub fn is_failure(&self) -> bool {
        !self.is_success()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VASSZReachSolverResult<T = (), F = ()> {
    pub status: SolverStatus<T, F>,
    pub duration: std::time::Duration,
}

impl VASSZReachSolverResult<(), ()> {
    pub fn from_bool(result: bool, duration: std::time::Duration) -> Self {
        Self {
            status: result.into(),
            duration,
        }
    }
}

impl<T, F> VASSZReachSolverResult<T, F> {
    pub fn new(result: SolverStatus<T, F>, duration: std::time::Duration) -> Self {
        Self {
            status: result,
            duration,
        }
    }

    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }

    pub fn is_failure(&self) -> bool {
        self.status.is_failure()
    }
}

impl VASSZReachSolverResult<ParikhImage, ()> {
    pub fn get_parikh_image(&self) -> Option<&ParikhImage> {
        match &self.status {
            SolverStatus::True(parikh_image) => Some(parikh_image),
            SolverStatus::False(_) => None,
        }
    }

    pub fn can_build_n_run<N: AutomatonNode>(
        &self,
        cfg: &VASSCFG<N>,
        initial_valuation: &[i32],
        final_valuation: &[i32],
    ) -> bool {
        let Some(parikh_image) = self.get_parikh_image() else {
            return false;
        };

        let valuation = initial_valuation.to_vec().into_boxed_slice();

        rec_can_build_n_run(
            parikh_image.clone(),
            valuation,
            final_valuation,
            cfg,
            cfg.get_start().expect("CFG has no start node"),
        )
    }

    pub fn can_build_z_run<N: AutomatonNode>(
        &self,
        cfg: &VASSCFG<N>,
        initial_valuation: &[i32],
        final_valuation: &[i32],
    ) -> bool {
        let Some(parikh_image) = self.get_parikh_image() else {
            return false;
        };

        let valuation = initial_valuation.to_vec().into_boxed_slice();

        rec_can_build_z_run(
            parikh_image.clone(),
            valuation,
            final_valuation,
            cfg,
            cfg.get_start().expect("CFG has no start node"),
        )
    }
}

fn rec_can_build_n_run<N: AutomatonNode>(
    parikh_image: ParikhImage,
    valuation: Box<[i32]>,
    final_valuation: &[i32],
    cfg: &VASSCFG<N>,
    node_index: NodeIndex,
) -> bool {
    let is_final = cfg.graph[node_index].accepting;
    // if the parikh image is empty, we have reached the end of the path, which also means that the path exists if the node is final
    if parikh_image.image.iter().all(|(_, v)| *v == 0) {
        assert_eq!(valuation.as_ref(), final_valuation);
        return is_final;
    }

    let outgoing = cfg
        .graph
        .edges_directed(node_index, petgraph::Direction::Outgoing);

    for edge in outgoing {
        // first we check that the edge can still be taken
        let edge_index = edge.id();
        let Some(edge_parikh) = parikh_image.image.get(&edge_index) else {
            continue;
        };
        if *edge_parikh == 0 {
            continue;
        }

        // next we check that taking the edge does not make a counter in the valuation negative
        let update = edge.weight();
        if valuation[update.counter()] + update.op() < 0 {
            continue;
        }

        // we can take the edge, so we update the parikh image and the valuation
        let mut valuation = valuation.clone();
        update.apply(&mut valuation);

        let mut parikh = parikh_image.clone();
        parikh.image.insert(edge_index, edge_parikh - 1);

        if rec_can_build_n_run(parikh, valuation, final_valuation, cfg, edge.target()) {
            return true;
        }
    }

    false
}

fn rec_can_build_z_run<N: AutomatonNode>(
    parikh_image: ParikhImage,
    valuation: Box<[i32]>,
    final_valuation: &[i32],
    cfg: &VASSCFG<N>,
    node_index: NodeIndex,
) -> bool {
    let is_final = cfg.graph[node_index].accepting;
    // if the parikh image is empty, we have reached the end of the path, which also means that the path exists if the node is final
    if parikh_image.image.iter().all(|(_, v)| *v == 0) {
        assert_eq!(valuation.as_ref(), final_valuation);
        return is_final;
    }

    let outgoing = cfg
        .graph
        .edges_directed(node_index, petgraph::Direction::Outgoing);

    for edge in outgoing {
        // first we check that the edge can still be taken
        let edge_index = edge.id();
        let Some(edge_parikh) = parikh_image.image.get(&edge_index) else {
            continue;
        };
        if *edge_parikh == 0 {
            continue;
        }

        let update = edge.weight();

        // we can take the edge, so we update the parikh image and the valuation
        let mut valuation = valuation.clone();
        update.apply(&mut valuation);

        let mut parikh = parikh_image.clone();
        parikh.image.insert(edge_index, edge_parikh - 1);

        if rec_can_build_n_run(parikh, valuation, final_valuation, cfg, edge.target()) {
            return true;
        }
    }

    false
}

/// Solves a VASS CFG for Z-Reachability.
///
/// The basic idea is to use a SAT solver to find a Z-Run through the CFG.
///
/// We create a variable for each edge in the CFG that represents how often the edge is taken.
/// Additionally we have one variable for each accepting node that represents whether the node is used as the final node.
/// We then create constraints that ensure that the sum of all incoming edges is equal to the sum of all outgoing edges for each node. (Kirchhoff Equations)
/// We also create constraints that ensure that the final valuation is equal to the sum of all edge valuations plus the initial valuation.
///
/// We then check if the constraints are satisfiable.
/// Due to the nature of the the Kirchhoff Equations, the Parikh Image generated by the solver may not form a connected Z-Run.
/// Should a solution not be connected, we add an additional constraint that forces:
///
/// > If all edges in a connected component are taken, then at least one outgoing edge (to a node that is not part of the connected component) must be taken as well.
///
/// This constraint ensures that the connected component must either be bigger or connected to the main Z-Run in the next iteration.
///
/// Since this constraint act's on sets of nodes and there are only a limited number of subsets of nodes, the solver terminates.
pub fn solve_z_reach_for_cfg<N: AutomatonNode>(
    cfg: &VASSCFG<N>,
    initial_valuation: &[i32],
    final_valuation: &[i32],
    logger: Option<&Logger>,
) -> VASSZReachSolverResult<ParikhImage, ()> {
    let time = std::time::Instant::now();
    let mut config = Config::new();
    config.set_model_generation(true);
    let ctx = Context::new(&config);
    let solver = Solver::new(&ctx);

    // a map that allows us to access the edge variables by their edge id
    let mut edge_map = HashMap::new();

    // all the counter sums along the path
    let mut sums: Box<[_]> = initial_valuation
        .iter()
        .map(|x| Int::from_i64(&ctx, *x as i64))
        .collect();

    for edge in cfg.graph.edge_references() {
        let edge_marking = edge.weight();

        // we need one variable for each edge
        let edge_var = Int::new_const(&ctx, format!("edge_{}", edge.id().index()));
        // CONSTRAINT: an edge can only be taken positive times
        solver.assert(&edge_var.ge(&Int::from_i64(&ctx, 0)));

        // add the edges effect to the counter sum
        let i = edge_marking.counter();
        sums[i] = &sums[i] + &edge_var * edge_marking.op_i64();

        edge_map.insert(edge.id(), edge_var);
    }

    let mut final_var_sum = Int::from_i64(&ctx, 0);

    for node in cfg.graph.node_indices() {
        let outgoing = cfg
            .graph
            .edges_directed(node, petgraph::Direction::Outgoing);
        let incoming = cfg
            .graph
            .edges_directed(node, petgraph::Direction::Incoming);

        let mut outgoing_sum = Int::from_i64(&ctx, 0);
        // the start node has one additional incoming connection
        let mut incoming_sum = if Some(node) == cfg.get_start() {
            Int::from_i64(&ctx, 1)
        } else {
            Int::from_i64(&ctx, 0)
        };

        if cfg.graph[node].accepting {
            // for each accepting node, we need some additional variable that denotes whether the node is used as the final node
            let final_var = Int::new_const(&ctx, format!("node_{}_final", node.index()));
            solver.assert(&final_var.ge(&Int::from_i64(&ctx, 0)));

            outgoing_sum += &final_var;
            final_var_sum += &final_var;
        }

        for edge in outgoing {
            let edge_var = edge_map.get(&edge.id()).unwrap();
            outgoing_sum += edge_var;
        }

        for edge in incoming {
            let edge_var = edge_map.get(&edge.id()).unwrap();
            incoming_sum += edge_var;
        }

        // CONSTRAINT: the sum of all outgoing edges must be equal to the sum of all incoming edges for each node
        solver.assert(&outgoing_sum._eq(&incoming_sum));
    }

    // CONSTRAINT: only one final variable can be set
    solver.assert(&final_var_sum._eq(&Int::from_i64(&ctx, 1)));

    // CONSTRAINT: the final valuation must be equal to the counter sums
    for (sum, target) in sums.iter().zip(final_valuation) {
        solver.assert(&sum._eq(&Int::from_i64(&ctx, *target as i64)));
    }

    let mut steps = 1;
    let result;

    loop {
        match solver.check() {
            z3::SatResult::Sat => {
                let model = solver.get_model();
                let model = model.unwrap();

                let parikh_image: HashMap<EdgeIndex, _> = edge_map
                    .iter()
                    .map(|(id, var)| {
                        (
                            *id,
                            model.get_const_interp(var).unwrap().as_u64().unwrap() as u32,
                        )
                    })
                    .filter(|(_, count)| *count > 0)
                    .collect();

                let parikh_image = ParikhImage::new(parikh_image);
                let (_, components) = parikh_image.clone().split_into_connected_components(cfg);

                if components.is_empty() {
                    result = SolverStatus::True(parikh_image);
                    break;
                }

                for component in components {
                    // bools that represent whether each individual edge in the component is taken
                    let edges = component
                        .iter_edges()
                        .map(|edge| edge_map.get(&edge).unwrap().ge(&Int::from_i64(&ctx, 1)))
                        .collect_vec();
                    let edges_ref = edges.iter().collect_vec();

                    // bool that represent whether each individual edge that is outgoing from the component is taken
                    let outgoing = component
                        .get_outgoing_edges(cfg)
                        .iter()
                        .map(|edge| edge_map.get(edge).unwrap().ge(&Int::from_i64(&ctx, 1)))
                        .collect_vec();
                    let outgoing_ref = outgoing.iter().collect_vec();

                    let edges_ast = Bool::and(&ctx, &edges_ref);
                    let outgoing_ast = Bool::or(&ctx, &outgoing_ref);

                    // CONSTRAINT: if all edges in the component are taken, then at least one outgoing edge must be taken as well
                    // this is because we need to leave the component.
                    solver.assert(&edges_ast.implies(&outgoing_ast));
                }

                steps += 1;
            }
            z3::SatResult::Unsat => {
                result = SolverStatus::False(());
                break;
            }
            z3::SatResult::Unknown => panic!("Solver returned unknown"),
        };
    }

    if let Some(l) = logger {
        l.debug(&format!("Solved Z-Reach in {} steps", steps));
    }

    VASSZReachSolverResult::new(result, time.elapsed())
}
