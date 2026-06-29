use std::{collections::VecDeque, time::Instant};

use hashbrown::{HashMap, HashSet};
use petgraph::graph::NodeIndex;

use super::{VASSReachSolverError, VASSReachSolverStatus};
use crate::{
    automaton::{
        Alphabet, GIndex, InitializedAutomaton, Letter, TransitionSystem,
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        dfa::minimization::Minimizable,
        linear_graph::{LinearGraph, part::LinearGraphRegion},
        path::Path,
        scc::{SCCAlgorithms, SCCDag, SCCDagEdge},
        vass::counter::VASSCounterValuation,
    },
    config::VASSReachConfig,
    solver::{SolverStatus, linear_graph_reach::LinearGraphReachSolverOptions},
};

type CFGPath = Path<NodeIndex, CFGCounterUpdate>;

pub(super) enum PreprocessOutcome {
    Refined(VASSCFG<()>),
    Reachable(CFGPath),
}

#[derive(Clone)]
struct AcceptingRoute<NIndex: GIndex, L: Letter> {
    edges: Vec<SCCDagEdge<NIndex, L>>,
    accepting: NIndex,
}

pub(super) fn run_preprocess_unreachable_linear_graph_from_scc_dag(
    cfg: VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    final_valuation: &VASSCounterValuation,
    config: &VASSReachConfig,
    solver_start_time: Option<Instant>,
) -> Result<PreprocessOutcome, VASSReachSolverStatus> {
    if !*config.get_preprocessing().get_enabled() {
        return Ok(PreprocessOutcome::Refined(cfg));
    }

    if !*config.get_linear_graph().get_enabled() {
        return Ok(PreprocessOutcome::Refined(cfg));
    }

    if !has_reachable_accepting(&cfg) {
        tracing::debug!(
            "Skipping LinearGraph preprocessing because CFG has no reachable accepting node"
        );
        return Ok(PreprocessOutcome::Refined(cfg));
    }

    max_time_reached(config, solver_start_time)?;

    let base_cfg = cfg;
    let dag = base_cfg.find_scc_dag().with_rolled_trivial_paths();
    let max_candidates = *config.get_preprocessing().get_max_linear_graph_candidates();
    let routes = collect_accepting_routes(&dag, max_candidates);

    if routes.is_empty() {
        tracing::debug!("No SCC-DAG LinearGraph preprocessing routes found");
        return Ok(PreprocessOutcome::Refined(base_cfg));
    }

    let mut processed_cfg = base_cfg.clone();

    tracing::info!(
        routes = routes.len(),
        "Running LinearGraph preprocessing over SCC-DAG routes"
    );

    let mut unreachable = 0usize;
    let mut reachable = 0usize;
    let mut unknown = 0usize;
    let mut skipped = 0usize;

    let dimension = initial_valuation.dimension();

    for route in routes {
        max_time_reached(config, solver_start_time)?;

        let Some(linear_graph) = build_linear_graph_from_route(
            &base_cfg,
            dimension,
            &dag,
            &route.edges,
            &route.accepting,
        ) else {
            skipped += 1;
            continue;
        };

        let solver_result = LinearGraphReachSolverOptions::default()
            .with_optional_time_limit(remaining_time(config, solver_start_time))
            .into_solver(&linear_graph, initial_valuation, final_valuation)
            .solve();

        match solver_result.status {
            SolverStatus::False(_) => {
                let mut cfg = linear_graph.to_cfg();
                cfg.invert_mut();
                processed_cfg = processed_cfg.intersect(&cfg);
                unreachable += 1;
            }
            SolverStatus::True(solution) => {
                if let Some(run) = solution.build_run_with_deadline(
                    &linear_graph,
                    true,
                    global_deadline(config, solver_start_time),
                ) && base_cfg.is_accepting(run.end())
                    && run.is_n_reaching(initial_valuation, final_valuation)
                {
                    tracing::info!(
                        run_length = run.len(),
                        "LinearGraph preprocessing produced a concrete N-reaching run"
                    );
                    return Ok(PreprocessOutcome::Reachable(run));
                }
                reachable += 1;
            }
            SolverStatus::Unknown(reason) => {
                if matches!(
                    reason,
                    crate::solver::linear_graph_reach::LinearGraphReachSolverError::Timeout
                ) && max_time_reached(config, solver_start_time).is_err()
                {
                    return Err(SolverStatus::Unknown(VASSReachSolverError::Timeout));
                }
                unknown += 1;
            }
        }

        max_time_reached(config, solver_start_time)?;
    }

    processed_cfg = processed_cfg.minimize();

    tracing::info!(
        unreachable,
        reachable,
        unknown,
        skipped,
        "Finished SCC-DAG LinearGraph preprocessing"
    );

    Ok(PreprocessOutcome::Refined(processed_cfg))
}

fn remaining_time(
    config: &VASSReachConfig,
    solver_start_time: Option<Instant>,
) -> Option<std::time::Duration> {
    let configured = *config.get_timeout();
    let remaining_global = match (configured, solver_start_time) {
        (Some(timeout), Some(started)) => Some(timeout.saturating_sub(started.elapsed())),
        (Some(timeout), None) => Some(timeout),
        (None, _) => None,
    };
    let per_check = *config.get_linear_graph().get_reach_solver_timeout();

    match (remaining_global, per_check) {
        (Some(global), Some(check)) => Some(global.min(check)),
        (Some(global), None) => Some(global),
        (None, Some(check)) => Some(check),
        (None, None) => None,
    }
}

fn global_deadline(
    config: &VASSReachConfig,
    solver_start_time: Option<Instant>,
) -> Option<Instant> {
    solver_start_time
        .zip(*config.get_timeout())
        .and_then(|(started, timeout)| started.checked_add(timeout))
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

fn build_linear_graph_from_route<'a>(
    cfg: &'a VASSCFG<()>,
    dimension: usize,
    dag: &SCCDag<NodeIndex, CFGCounterUpdate>,
    route: &[SCCDagEdge<NodeIndex, CFGCounterUpdate>],
    accepting_node: &NodeIndex,
) -> Option<LinearGraph<'a, NodeIndex, VASSCFG<()>>> {
    let mut component_indices = Vec::with_capacity(route.len() + 1);
    component_indices.push(dag.root_component);
    component_indices.extend(route.iter().map(|edge| edge.target_component));

    let mut linear_graph = LinearGraph::empty(cfg, dimension);
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
                linear_graph.add_path(current_path.clone().into());
            }

            linear_graph.add_graph(LinearGraphRegion::from_subset(
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
        linear_graph.add_path(current_path.into());
    }

    Some(linear_graph)
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
