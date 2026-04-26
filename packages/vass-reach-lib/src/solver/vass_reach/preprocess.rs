use std::{collections::VecDeque, time::Instant};

use hashbrown::{HashMap, HashSet};
use petgraph::graph::NodeIndex;

use super::{VASSReachSolverError, VASSReachSolverStatus};
use crate::{
    automaton::{
        Alphabet, GIndex, InitializedAutomaton, Letter, TransitionSystem,
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        dfa::minimization::Minimizable,
        mgts::{MGTS, part::MarkedGraph},
        path::Path,
        scc::{SCCAlgorithms, SCCDag, SCCDagEdge},
        vass::counter::VASSCounterValuation,
    },
    config::VASSReachConfig,
    solver::{SolverStatus, mgts_reach::MGTSReachSolverOptions},
};

type CFGPath = Path<NodeIndex, CFGCounterUpdate>;

#[derive(Clone)]
struct AcceptingRoute<NIndex: GIndex, L: Letter> {
    edges: Vec<SCCDagEdge<NIndex, L>>,
    accepting: NIndex,
}

pub(super) fn run_preprocess_unreachable_mgts_from_scc_dag(
    cfg: VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    final_valuation: &VASSCounterValuation,
    config: &VASSReachConfig,
    solver_start_time: Option<Instant>,
) -> Result<VASSCFG<()>, VASSReachSolverStatus> {
    if !*config.get_preprocessing().get_enabled() {
        return Ok(cfg);
    }

    if !*config.get_mgts().get_enabled() {
        return Ok(cfg);
    }

    if !has_reachable_accepting(&cfg) {
        tracing::debug!("Skipping MGTS preprocessing because CFG has no reachable accepting node");
        return Ok(cfg);
    }

    max_time_reached(config, solver_start_time)?;

    let base_cfg = cfg;
    let dag = base_cfg.find_scc_dag().with_rolled_trivial_paths();
    let max_candidates = *config.get_preprocessing().get_max_mgts_candidates();
    let routes = collect_accepting_routes(&dag, max_candidates);

    if routes.is_empty() {
        tracing::debug!("No SCC-DAG MGTS preprocessing routes found");
        return Ok(base_cfg);
    }

    let mut processed_cfg = base_cfg.clone();

    tracing::info!(
        routes = routes.len(),
        "Running MGTS preprocessing over SCC-DAG routes"
    );

    let mut unreachable = 0usize;
    let mut reachable = 0usize;
    let mut unknown = 0usize;
    let mut skipped = 0usize;

    let dimension = initial_valuation.dimension();

    for route in routes {
        max_time_reached(config, solver_start_time)?;

        let Some(mgts) =
            build_mgts_from_route(&base_cfg, dimension, &dag, &route.edges, &route.accepting)
        else {
            skipped += 1;
            continue;
        };

        let solver_result = MGTSReachSolverOptions::default()
            .to_solver(&mgts, initial_valuation, final_valuation)
            .solve();

        match solver_result.status {
            SolverStatus::False(_) => {
                let mut cfg = mgts.to_cfg();
                cfg.invert_mut();
                processed_cfg = processed_cfg.intersect(&cfg);
                unreachable += 1;
            }
            SolverStatus::True(_) => {
                reachable += 1;
            }
            SolverStatus::Unknown(_) => {
                unknown += 1;
            }
        }
    }

    processed_cfg = processed_cfg.minimize();

    tracing::info!(
        unreachable,
        reachable,
        unknown,
        skipped,
        "Finished SCC-DAG MGTS preprocessing"
    );

    Ok(processed_cfg)
}

fn collect_accepting_routes<NIndex: GIndex, L: Letter>(
    dag: &SCCDag<NIndex, L>,
    max_routes: usize,
) -> Vec<AcceptingRoute<NIndex, L>> {
    #[derive(Clone)]
    struct StackEntry<NIndex: GIndex, L: Letter> {
        component: usize,
        route: Vec<SCCDagEdge<NIndex, L>>,
    }

    let mut routes = Vec::new();
    let mut stack = vec![StackEntry {
        component: dag.root_component,
        route: Vec::new(),
    }];

    while let Some(entry) = stack.pop() {
        if routes.len() >= max_routes {
            break;
        }

        if !dag.components[entry.component].accepting_nodes.is_empty() {
            for accepting in &dag.components[entry.component].accepting_nodes {
                if routes.len() >= max_routes {
                    break;
                }

                routes.push(AcceptingRoute {
                    edges: entry.route.clone(),
                    accepting: accepting.clone(),
                });
            }
        }

        for edge in dag.outgoing_edges(entry.component).iter().rev() {
            if routes.len() >= max_routes {
                break;
            }

            let mut next_route = entry.route.clone();
            next_route.push(edge.clone());

            stack.push(StackEntry {
                component: edge.target_component,
                route: next_route,
            });
        }
    }

    routes
}

fn build_mgts_from_route<'a>(
    cfg: &'a VASSCFG<()>,
    dimension: usize,
    dag: &SCCDag<NodeIndex, CFGCounterUpdate>,
    route: &[SCCDagEdge<NodeIndex, CFGCounterUpdate>],
    accepting_node: &NodeIndex,
) -> Option<MGTS<'a, NodeIndex, VASSCFG<()>>> {
    let mut component_indices = Vec::with_capacity(route.len() + 1);
    component_indices.push(dag.root_component);
    component_indices.extend(route.iter().map(|edge| edge.target_component));

    let mut mgts = MGTS::empty(cfg, dimension);
    let mut current_path = CFGPath::new(cfg.get_initial());

    let mut entry_node = cfg.get_initial();

    for (component_position, component_index) in component_indices.iter().enumerate() {
        let component = &dag.components[*component_index];
        let outgoing_edge = route.get(component_position);

        let (exit_node, connector) = if let Some(edge) = outgoing_edge {
            let exit = *edge.path.start();
            let connector =
                find_path_within_component_cfg(cfg, &entry_node, &exit, &component.nodes)?;
            (exit, connector)
        } else {
            let connector =
                find_path_within_component_cfg(cfg, &entry_node, accepting_node, &component.nodes)?;
            (*accepting_node, connector)
        };

        if component.is_trivial() {
            current_path.concat(connector);
        } else {
            if !current_path.is_empty() {
                mgts.add_path(current_path.clone().into());
            }

            mgts.add_graph(MarkedGraph::from_subset(
                cfg,
                &component.nodes,
                entry_node,
                exit_node,
            ));

            current_path = CFGPath::new(exit_node);
        }

        if let Some(edge) = outgoing_edge {
            current_path.concat(edge.path.clone());
            entry_node = *edge.path.end();
        }
    }

    if !current_path.is_empty() {
        mgts.add_path(current_path.into());
    }

    Some(mgts)
}

fn find_path_within_component_cfg(
    cfg: &VASSCFG<()>,
    start: &NodeIndex,
    end: &NodeIndex,
    nodes: &[NodeIndex],
) -> Option<CFGPath> {
    if start == end {
        return Some(CFGPath::new(*start));
    }

    let allowed = nodes.iter().cloned().collect::<HashSet<_>>();
    if !allowed.contains(start) || !allowed.contains(end) {
        return None;
    }

    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    let mut parent = HashMap::<NodeIndex, (NodeIndex, CFGCounterUpdate)>::new();

    queue.push_back(*start);
    visited.insert(*start);

    while let Some(current) = queue.pop_front() {
        for letter in cfg.alphabet() {
            let Some(next) = cfg.successor(&current, letter) else {
                continue;
            };

            if !allowed.contains(&next) || !visited.insert(next) {
                continue;
            }

            parent.insert(next, (current, *letter));

            if &next == end {
                return Some(reconstruct_cfg_path(*start, *end, &parent));
            }

            queue.push_back(next);
        }
    }

    None
}

fn reconstruct_cfg_path(
    start: NodeIndex,
    end: NodeIndex,
    parent: &HashMap<NodeIndex, (NodeIndex, CFGCounterUpdate)>,
) -> CFGPath {
    let mut reversed = Vec::new();
    let mut cursor = end;

    while cursor != start {
        let (previous, letter) = *parent
            .get(&cursor)
            .expect("BFS parent map missing edge for reconstructed path");
        reversed.push((letter, cursor));
        cursor = previous;
    }

    reversed.reverse();

    let mut path = CFGPath::new(start);
    for (letter, state) in reversed {
        path.add(letter, state);
    }

    path
}
fn has_reachable_accepting(cfg: &VASSCFG<()>) -> bool {
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    let initial = cfg.get_initial();

    queue.push_back(initial);
    visited.insert(initial);

    while let Some(current) = queue.pop_front() {
        if cfg.is_accepting(&current) {
            return true;
        }

        for letter in cfg.alphabet() {
            let Some(next) = cfg.successor(&current, letter) else {
                continue;
            };
            if visited.insert(next) {
                queue.push_back(next);
            }
        }
    }

    false
}

fn max_time_reached(
    config: &VASSReachConfig,
    solver_start_time: Option<Instant>,
) -> Result<(), VASSReachSolverStatus> {
    if let Some(start_time) = solver_start_time
        && let Some(max_time) = config.get_timeout()
        && start_time.elapsed() > *max_time
    {
        return Err(SolverStatus::Unknown(VASSReachSolverError::Timeout));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{has_reachable_accepting, run_preprocess_unreachable_mgts_from_scc_dag};
    use crate::{
        automaton::{
            Automaton, ExplicitEdgeAutomaton, ModifiableAutomaton,
            dfa::minimization::Minimizable,
            vass::{VASS, VASSEdge},
        },
        config::VASSReachConfig,
    };

    #[test]
    fn preprocessing_disabled_keeps_intersection_unchanged() {
        let mut vass = VASS::<u32, char>::new(1, vec!['a']);
        let q0 = vass.add_node(0);
        let q1 = vass.add_node(1);
        vass.add_edge(&q0, &q1, VASSEdge::new('a', vec![0].into()));

        let initialized_vass = vass.init(vec![0].into(), vec![0].into(), q0, q1);

        let mut cfg = initialized_vass.to_cfg();
        cfg.make_complete(());
        cfg = cfg.minimize();

        let before_nodes = cfg.node_count();
        let before_edges = cfg.edge_count();

        let processed = run_preprocess_unreachable_mgts_from_scc_dag(
            cfg,
            &initialized_vass.initial_valuation,
            &initialized_vass.final_valuation,
            &VASSReachConfig::default().with_preprocessing(
                crate::config::PreprocessingConfig::default().with_enabled(false),
            ),
            None,
        )
        .unwrap();

        assert_eq!(processed.node_count(), before_nodes);
        assert_eq!(processed.edge_count(), before_edges);
    }

    #[test]
    fn preprocessing_cuts_all_accepting_routes_in_difficult_instance() {
        let mut vass = VASS::new(2, (0..10).collect());

        let s0 = vass.add_node(());
        let s1 = vass.add_node(());
        let s2 = vass.add_node(());
        let s3 = vass.add_node(());

        vass.add_edge(&s0, &s1, VASSEdge::new(0, vec![6, 0].into()));

        vass.add_edge(&s1, &s1, VASSEdge::new(1, vec![1, 1].into()));
        vass.add_edge(&s1, &s1, VASSEdge::new(2, vec![-1, -1].into()));
        vass.add_edge(&s1, &s1, VASSEdge::new(3, vec![1, 0].into()));

        vass.add_edge(&s1, &s2, VASSEdge::new(4, vec![0, 0].into()));

        vass.add_edge(&s2, &s2, VASSEdge::new(5, vec![1, 2].into()));
        vass.add_edge(&s2, &s2, VASSEdge::new(6, vec![-1, -2].into()));

        vass.add_edge(&s2, &s3, VASSEdge::new(7, vec![0, 0].into()));

        vass.add_edge(&s3, &s3, VASSEdge::new(8, vec![0, 1].into()));
        vass.add_edge(&s3, &s3, VASSEdge::new(9, vec![0, -1].into()));

        let initialized = vass.init(vec![0, 0].into(), vec![0, 0].into(), s0, s3);

        let mut cfg = initialized.to_cfg();
        cfg.make_complete(());
        cfg = cfg.minimize();

        let processed = run_preprocess_unreachable_mgts_from_scc_dag(
            cfg,
            &initialized.initial_valuation,
            &initialized.final_valuation,
            &VASSReachConfig::default().with_preprocessing(
                crate::config::PreprocessingConfig::default().with_enabled(true),
            ),
            None,
        )
        .unwrap();

        assert!(!has_reachable_accepting(&processed));
    }
}
