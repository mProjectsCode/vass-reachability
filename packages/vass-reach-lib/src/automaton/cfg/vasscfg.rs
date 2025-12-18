use petgraph::{Direction, graph::NodeIndex, visit::EdgeRef};

use crate::automaton::{
    Automaton, AutomatonNode, InitializedAutomaton, ModifiableAutomaton,
    cfg::{
        CFG,
        update::{CFGCounterUpdatable, CFGCounterUpdate},
    },
    dfa::{DFA, node::DfaNode},
    path::Path,
    vass::counter::{VASSCounterIndex, VASSCounterValuation},
};

pub type VASSCFG<N> = DFA<N, CFGCounterUpdate>;

impl<N: AutomatonNode> CFG for VASSCFG<N> {}

impl<N: AutomatonNode> VASSCFG<N> {
    /// Find a reaching paths though the CFG while only counting the counters
    /// modulo `mu`. If a path is found, it is the shortest possible
    /// reaching path with the given modulo.
    ///
    /// Since the number of possible counter valuations is finite, this function
    /// is guaranteed to terminate.
    pub fn modulo_reach(
        &self,
        mu: i32,
        initial_valuation: &VASSCounterValuation,
        final_valuation: &VASSCounterValuation,
    ) -> Option<Path<NodeIndex, CFGCounterUpdate>> {
        // For every node, we track which counter valuations we already visited.
        let mut visited = vec![std::collections::HashSet::new(); self.node_count()];
        let mut queue = std::collections::VecDeque::new();
        let mut mod_initial_valuation = initial_valuation.clone();
        let mut mod_final_valuation = final_valuation.clone();
        mod_initial_valuation.mod_euclid_mut(mu);
        mod_final_valuation.mod_euclid_mut(mu);

        let start = self.get_initial();
        let initial_path = Path::new(start);
        if self.graph[start].accepting && mod_initial_valuation == mod_final_valuation {
            return Some(initial_path);
        }

        queue.push_back((initial_path, mod_initial_valuation.clone()));
        visited[start.index()].insert(mod_initial_valuation);

        while let Some((path, valuation)) = queue.pop_front() {
            let last = path.end();

            for edge in self.graph.edges_directed(last, Direction::Outgoing) {
                let mut new_valuation = valuation.clone();

                let update = edge.weight();
                new_valuation.apply_cfg_update_mod(*update, mu);

                let target = edge.target();

                if visited[target.index()].insert(new_valuation.clone()) {
                    let mut new_path = path.clone();
                    new_path.add(*update, target);

                    if self.graph[target].accepting && new_valuation == mod_final_valuation {
                        // paths.push(new_path);
                        // Optimization: we only search for the shortest path, so we can stop when
                        // we find one
                        return Some(new_path);
                    } else {
                        queue.push_back((new_path, new_valuation));
                    }
                }
            }
        }

        None
    }

    pub fn reverse_counter_updates(&mut self) {
        for edge in self.graph.edge_weights_mut() {
            *edge = edge.reverse();
        }
    }
}

pub fn build_bounded_counting_cfg(
    dimension: usize,
    counter: VASSCounterIndex,
    limit: u32,
    start: i32,
    end: i32,
) -> VASSCFG<()> {
    // if limit == 0 {
    //     panic!("Limit must be greater than 0");
    // }

    let counter_up = CFGCounterUpdate::positive(counter);
    let counter_down = CFGCounterUpdate::negative(counter);

    let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(dimension));

    let negative = cfg.add_node(DfaNode::new(false, true, ()));
    let overflow = cfg.add_node(DfaNode::accepting(()));

    // once negative always stays negative
    for c in CFGCounterUpdate::alphabet(dimension) {
        cfg.add_edge(negative, negative, c);
        cfg.add_edge(overflow, overflow, c);
    }

    let mut states = vec![negative];
    states.extend((0..=limit).map(|i| cfg.add_node(DfaNode::new(i == end as u32, false, ()))));
    states.push(overflow);

    for i in 1..states.len() - 1 {
        let prev = states[i - 1];
        let current = states[i];
        let next = states[i + 1];
        cfg.add_edge(current, prev, counter_down);
        cfg.add_edge(current, next, counter_up);

        for c in CFGCounterUpdate::alphabet(dimension) {
            if c != counter_up && c != counter_down {
                cfg.add_edge(current, current, c);
            }
        }

        if i as i32 == start + 1 {
            cfg.set_initial(current);
        }
    }

    #[cfg(debug_assertions)]
    cfg.assert_complete();

    cfg.set_complete_unchecked();

    cfg
}

pub fn build_rev_bounded_counting_cfg(
    dimension: usize,
    counter: VASSCounterIndex,
    limit: u32,
    start: i32,
    end: i32,
) -> DFA<(), CFGCounterUpdate> {
    let cfg = build_bounded_counting_cfg(dimension, counter, limit, end, start);

    let mut cfg = cfg.reverse();
    cfg.reverse_counter_updates();

    cfg
}
