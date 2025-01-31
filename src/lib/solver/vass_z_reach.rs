use hashbrown::HashMap;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use z3::{
    ast::{Ast, Int},
    Config, Context, Solver,
};

use crate::automaton::{dfa::DFA, vass::InitializedVASS, AutEdge, AutNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VASSZReachSolverResult {
    pub result: bool,
    pub duration: std::time::Duration,
}

impl VASSZReachSolverResult {
    pub fn new(result: bool, duration: std::time::Duration) -> Self {
        Self { result, duration }
    }
}

pub fn solve_z_reach<N: AutNode, E: AutEdge>(
    ivass: &InitializedVASS<N, E>,
) -> VASSZReachSolverResult {
    let time = std::time::Instant::now();
    let config = Config::new();
    let ctx = Context::new(&config);
    let solver = Solver::new(&ctx);

    let zero = Int::from_i64(&ctx, 0);

    // a map that allows us to access the edge variables by their edge id
    let mut edge_map = HashMap::new();

    // all the counter sums along the path
    let mut sums: Box<[_]> = ivass
        .initial_valuation
        .iter()
        .map(|x| Int::from_i64(&ctx, *x as i64))
        .collect();

    for edge in ivass.vass.graph.edge_references() {
        let edge_marking = &edge.weight().1;

        // we need one variable for each edge
        let edge_var = Int::new_const(&ctx, format!("edge_{}", edge.id().index()).as_str());
        // CONSTRAINT: an edge can only be taken positive times
        solver.assert(&edge_var.ge(&zero));

        // add the edges effect to the counter sum
        for i in 0..ivass.vass.dimension {
            sums[i] = &sums[i] + &Int::from_i64(&ctx, edge_marking[i] as i64) * &edge_var;
        }

        edge_map.insert(edge.id(), edge_var);
    }

    for node in ivass.vass.graph.node_indices() {
        let outgoing = ivass
            .vass
            .graph
            .edges_directed(node, petgraph::Direction::Outgoing);
        let incoming = ivass
            .vass
            .graph
            .edges_directed(node, petgraph::Direction::Incoming);

        let mut outgoing_sum = Int::from_i64(&ctx, 0);
        let mut incoming_sum = Int::from_i64(&ctx, 0);

        for edge in outgoing {
            let edge_var = edge_map.get(&edge.id()).unwrap();
            outgoing_sum = &outgoing_sum + edge_var;
        }

        for edge in incoming {
            let edge_var = edge_map.get(&edge.id()).unwrap();
            incoming_sum = &incoming_sum + edge_var;
        }

        // CONSTRAINT: the sum of all outgoing edges must be equal to the sum of all incoming edges for each node
        solver.assert(&outgoing_sum._eq(&incoming_sum));
    }

    // CONSTRAINT: the final valuation must be equal to the counter sums
    for (sum, target) in sums.iter().zip(&ivass.final_valuation) {
        solver.assert(&sum._eq(&Int::from_i64(&ctx, *target as i64)));
    }

    let result = match solver.check() {
        z3::SatResult::Sat => true,
        z3::SatResult::Unsat => false,
        z3::SatResult::Unknown => panic!("Solver returned unknown"),
    };

    VASSZReachSolverResult::new(result, time.elapsed())
}

pub fn solve_z_reach_for_cfg<N: AutNode>(
    cfg: &DFA<N, i32>,
    initial_valuation: &[i32],
    final_valuation: &[i32],
) -> VASSZReachSolverResult {
    let time = std::time::Instant::now();
    let config = Config::new();
    let ctx = Context::new(&config);
    let solver = Solver::new(&ctx);
    let zero = Int::from_i64(&ctx, 0);

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
        let edge_var = Int::new_const(&ctx, format!("edge_{}", edge.id().index()).as_str());
        // CONSTRAINT: an edge can only be taken positive times
        solver.assert(&edge_var.ge(&zero));

        // add the edges effect to the counter sum
        let i = (edge_marking.unsigned_abs() - 1) as usize;
        let sign = if edge_marking.is_negative() {
            -1i64
        } else {
            1i64
        };
        sums[i] = &sums[i] + &edge_var * sign;

        edge_map.insert(edge.id(), edge_var);
    }

    for node in cfg.graph.node_indices() {
        let outgoing = cfg
            .graph
            .edges_directed(node, petgraph::Direction::Outgoing);
        let incoming = cfg
            .graph
            .edges_directed(node, petgraph::Direction::Incoming);

        let mut outgoing_sum = Int::from_i64(&ctx, 0);
        let mut incoming_sum = Int::from_i64(&ctx, 0);

        for edge in outgoing {
            let edge_var = edge_map.get(&edge.id()).unwrap();
            outgoing_sum = &outgoing_sum + edge_var;
        }

        for edge in incoming {
            let edge_var = edge_map.get(&edge.id()).unwrap();
            incoming_sum = &incoming_sum + edge_var;
        }

        // CONSTRAINT: the sum of all outgoing edges must be equal to the sum of all incoming edges for each node
        solver.assert(&outgoing_sum._eq(&incoming_sum));
    }

    // CONSTRAINT: the final valuation must be equal to the counter sums
    for (sum, target) in sums.iter().zip(final_valuation) {
        solver.assert(&sum._eq(&Int::from_i64(&ctx, *target as i64)));
    }

    let result = match solver.check() {
        z3::SatResult::Sat => true,
        z3::SatResult::Unsat => false,
        z3::SatResult::Unknown => panic!("Solver returned unknown"),
    };

    VASSZReachSolverResult::new(result, time.elapsed())
}
