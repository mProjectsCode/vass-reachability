use std::fmt::Display;

use crate::automaton::{
    AutomatonEdge, Deterministic, ExplicitEdgeAutomaton, GIndex, Letter, TransitionSystem,
    cfg::update::{CFGCounterUpdatable, CFGCounterUpdate},
    vass::counter::{VASSCounterIndex, VASSCounterValuation},
};

pub mod parikh_image;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path<N: GIndex, L: Letter> {
    pub states: Vec<N>,
    pub transitions: Vec<L>,
}

impl<N: GIndex, L: Letter> Path<N, L> {
    pub fn new(start_index: N) -> Self {
        Path {
            states: vec![start_index],
            transitions: vec![],
        }
    }

    pub fn from_word<'a>(
        start_index: N,
        word: impl IntoIterator<Item = &'a L>,
        graph: &impl TransitionSystem<Deterministic, NIndex = N, Letter = L>,
    ) -> anyhow::Result<Self>
    where
        L: 'a,
    {
        let mut path = Path::new(start_index);

        for letter in word {
            path.take_edge(letter.clone(), graph)?;
        }

        Ok(path)
    }

    pub fn add(&mut self, letter: L, node: N) {
        self.transitions.push(letter);
        self.states.push(node);
    }

    pub fn take_edge(
        &mut self,
        letter: L,
        graph: &impl TransitionSystem<Deterministic, NIndex = N, Letter = L>,
    ) -> anyhow::Result<()> {
        let successor = graph.successor(self.end(), &letter).ok_or_else(|| {
            anyhow::anyhow!(format!(
                "path failed to take letter {:?}, no suitable successor found for end node {:?}",
                letter,
                self.end()
            ))
        })?;
        self.add(letter, successor);
        Ok(())
    }

    /// Checks if a path has a loop by checking if a node is visited twice
    pub fn has_loop(&self) -> bool {
        let mut visited = hashbrown::HashSet::new();
        for node in &self.states {
            if !visited.insert(node) {
                return true;
            }
        }
        false
    }

    pub fn start(&self) -> &N {
        &self.states[0]
    }

    pub fn end(&self) -> &N {
        self.states.last().unwrap()
    }

    pub fn len(&self) -> usize {
        debug_assert!(self.states.len() == self.transitions.len() + 1);
        self.transitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transitions.is_empty()
    }

    pub fn contains_node(&self, node: &N) -> bool {
        self.states.contains(node)
    }

    pub fn contains_state(&self, state: &N) -> bool {
        self.states.contains(state)
    }

    pub fn has_node(&self, node: &N) -> bool {
        self.contains_node(node)
    }

    pub fn state_len(&self) -> usize {
        self.states.len()
    }

    pub fn get_node(&self, index: usize) -> &N {
        &self.states[index + 1]
    }

    pub fn get_letter(&self, index: usize) -> &L {
        &self.transitions[index]
    }

    pub fn iter_letters(&self) -> impl Iterator<Item = &L> {
        self.transitions.iter()
    }

    pub fn iter_nodes(&self) -> impl Iterator<Item = &N> {
        self.states.iter()
    }

    pub fn iter_states(&self) -> impl Iterator<Item = &N> {
        self.states.iter()
    }

    pub fn iter<'a>(&'a self) -> impl DoubleEndedIterator<Item = (&'a L, &'a N)>
    where
        L: 'a,
        N: 'a,
    {
        self.transitions.iter().zip(self.states.iter().skip(1))
    }

    pub fn first(&self) -> Option<(&L, &N)> {
        self.transitions.first().zip(self.states.get(1))
    }

    pub fn last(&self) -> Option<(&L, &N)> {
        self.transitions.last().zip(self.states.last())
    }

    pub fn concat(&mut self, mut other: Self) {
        assert_eq!(
            self.end(),
            other.start(),
            "Paths can only be concatenated if the end of the first matches the start of the second"
        );
        self.transitions.append(&mut other.transitions);
        self.states.append(&mut other.states[1..].to_vec());
    }

    pub fn split_at_node(self, node: &N) -> Vec<Self> {
        if self.transitions.is_empty() || !self.contains_node(node) {
            return vec![self];
        }

        let mut parts = vec![];
        let mut current_part = Path::new(self.start().clone());

        for (letter, target) in self.iter() {
            current_part.add(letter.clone(), target.clone());

            if target == node {
                parts.push(current_part);
                current_part = Path::new(node.clone());
            }
        }

        if !current_part.is_empty() {
            parts.push(current_part);
        }

        parts
    }

    pub fn split_at_nodes(self, nodes: &[N]) -> Vec<Self> {
        if self.transitions.is_empty() || nodes.iter().all(|n| !self.contains_node(n)) {
            return vec![self];
        }

        let mut parts = vec![];
        let mut current_part = Path::new(self.start().clone());

        for (letter, target) in self.iter() {
            current_part.add(letter.clone(), target.clone());

            for node in nodes {
                if *node == *target {
                    parts.push(current_part);
                    current_part = Path::new(node.clone());
                    break;
                }
            }
        }

        parts.push(current_part);

        parts
    }

    pub fn split_at(self, f: impl Fn(&N, usize) -> bool) -> Vec<Self> {
        let mut parts = vec![];
        let mut current_part = Path::new(self.start().clone());

        for (i, (letter, state)) in self.iter().enumerate() {
            current_part.add(letter.clone(), state.clone());

            if f(state, i) {
                parts.push(current_part);
                current_part = Path::new(state.clone());
            }
        }

        parts.push(current_part);

        parts
    }

    pub fn slice(&self, range: std::ops::Range<usize>) -> Self {
        Self {
            transitions: self.transitions[range.clone()].to_vec(),
            states: self.states[range.start..=range.end].to_vec(),
        }
    }

    pub fn slice_end(&self, start: usize) -> Self {
        self.slice(start..self.len())
    }

    pub fn split_off(&mut self, i: usize) -> Self {
        debug_assert!(i < self.transitions.len());

        let mut new_path = Path::new(self.states[i + 1].clone());
        new_path.transitions = self.transitions.split_off(i);
        new_path.states.extend(self.states.split_off(i + 1));

        debug_assert!(self.states.len() == self.transitions.len() + 1);
        debug_assert!(new_path.states.len() == new_path.transitions.len() + 1);

        new_path
    }

    pub fn visited_edges<
        E: AutomatonEdge<Letter = L>,
        A: ExplicitEdgeAutomaton<Deterministic, NIndex = N, Letter = L, E = E>,
    >(
        &self,
        graph: &A,
    ) -> hashbrown::HashSet<A::EIndex> {
        let mut edges = hashbrown::HashSet::new();
        let mut current_node = self.start();

        for (letter, target) in self.iter() {
            let con_edges: Vec<_> = graph
                .connecting_edge_indices_with_letter(current_node, target, letter)
                .collect();
            if con_edges.is_empty() {
                panic!(
                    "No edge found for transition {:?} --({:?})-> {:?}",
                    current_node, letter, target
                );
            }
            if con_edges.len() > 1 {
                panic!(
                    "Multiple edges found for transition {:?} --({:?})-> {:?}, edges: {:?}",
                    current_node, letter, target, con_edges
                );
            }
            edges.insert(con_edges[0]);

            current_node = target;
        }

        edges
    }

    pub fn to_fancy_string(&self) -> String {
        format!("{}", self)
    }
}

impl<N: GIndex, L: Letter> Path<N, L>
where
    L: IntoIterator<Item = CFGCounterUpdate> + Clone,
{
    /// Checks if a path is N-reaching.
    pub fn is_n_reaching(
        &self,
        initial_valuation: &VASSCounterValuation,
        final_valuation: &VASSCounterValuation,
    ) -> bool {
        let mut counters = initial_valuation.clone();

        for edge in &self.transitions {
            for update in edge.clone() {
                counters.apply_cfg_update(update);

                if counters.has_negative_counter() {
                    return false;
                }
            }
        }

        &counters == final_valuation
    }
}

impl<N: GIndex> Path<N, CFGCounterUpdate> {
    /// Checks if a path is N-reaching.
    pub fn is_n_reaching(
        &self,
        initial_valuation: &VASSCounterValuation,
        final_valuation: &VASSCounterValuation,
    ) -> bool {
        let mut counters = initial_valuation.clone();

        for edge in &self.transitions {
            counters.apply_cfg_update(*edge);

            if counters.has_negative_counter() {
                return false;
            }
        }

        &counters == final_valuation
    }

    pub fn get_path_final_valuation(
        &self,
        initial_valuation: &VASSCounterValuation,
    ) -> VASSCounterValuation {
        let mut counters = initial_valuation.clone();
        for edge in &self.transitions {
            counters.apply_cfg_update(*edge);
        }
        counters
    }

    pub fn find_negative_counter_forward(
        &self,
        initial_valuation: &VASSCounterValuation,
    ) -> Option<(VASSCounterIndex, usize)> {
        let mut counters = initial_valuation.clone();

        for (i, edge) in self.transitions.iter().enumerate() {
            counters.apply_cfg_update(*edge);

            if let Some(counter) = counters.find_negative_counter() {
                return Some((counter, i));
            }
        }

        None
    }

    pub fn find_negative_counter_backward(
        &self,
        final_valuation: &VASSCounterValuation,
    ) -> Option<(VASSCounterIndex, usize)> {
        let mut counters = final_valuation.clone();

        for (i, edge) in self.transitions.iter().enumerate().rev() {
            counters.apply_cfg_update(edge.reverse());

            if let Some(counter) = counters.find_negative_counter() {
                return Some((counter, i));
            }
        }

        None
    }

    pub fn max_counter_value(
        &self,
        initial_valuation: &VASSCounterValuation,
        counter: VASSCounterIndex,
    ) -> i32 {
        let counter_updates = self
            .transitions
            .iter()
            .filter(|update| update.counter() == counter);

        let mut value = initial_valuation[counter];
        let mut max_value = initial_valuation[counter];
        for update in counter_updates {
            value += update.op();
            max_value = max_value.max(value);
        }

        max_value
    }

    pub fn max_counter_value_from_back(
        &self,
        final_valuation: &VASSCounterValuation,
        counter: VASSCounterIndex,
    ) -> i32 {
        let counter_updates = self
            .transitions
            .iter()
            .rev()
            .filter(|update| update.counter() == counter);

        let mut value = final_valuation[counter];
        let mut max_value = final_valuation[counter];
        for update in counter_updates {
            value -= update.op();
            max_value = max_value.max(value);
        }

        max_value
    }

    pub fn visits_node_multiple_times(&self, limit: u32) -> bool {
        let mut visited = hashbrown::HashMap::new();
        visited.insert(self.states[0].clone(), 1);

        for state in self.states.iter().skip(1) {
            let value = visited.entry(state.clone()).or_insert(0);
            *value += 1;
            if *value > limit {
                return true;
            }
        }

        false
    }
}

use crate::automaton::implicit_cfg_product::state::MultiGraphState;

impl Path<MultiGraphState, CFGCounterUpdate> {
    pub fn to_path_in_cfg(
        &self,
        cfg_index: usize,
    ) -> Path<petgraph::graph::NodeIndex, CFGCounterUpdate> {
        let mut path = Path::new(self.states[0].cfg_state(cfg_index));

        for (update, state) in self.iter() {
            path.add(*update, state.cfg_state(cfg_index));
        }

        path
    }

    pub fn is_counter_forwards_pumped(
        &self,
        dimension: usize,
        counter: VASSCounterIndex,
        limit: u32,
    ) -> bool {
        let mut visited = hashbrown::HashMap::new();
        let mut counters = VASSCounterValuation::zero(dimension);
        let mut start = self.states[0].clone();

        // clear the indices corresponding to the modulo and bounded counting
        // separators, since they don't matter for pumping
        start.clear_indices(1..dimension * 3 + 1);

        visited.insert(start, (1, counters.clone()));

        for (update, state) in self.iter() {
            counters.apply_cfg_update(*update);

            // clear the indices corresponding to the modulo and bounded counting
            // separators, since they don't matter for pumping
            let mut state = state.clone();
            state.clear_indices(1..dimension * 3 + 1);

            let entry = visited
                .entry(state)
                .or_insert((0, VASSCounterValuation::zero(dimension)));

            // check that we have pumped and that we pumped the counter we care about
            if counters >= entry.1 && counters[counter] > entry.1[counter] {
                entry.0 += 1;
                entry.1 = counters.clone();
                if entry.0 > limit {
                    return true;
                }
            }
        }

        false
    }

    pub fn is_counter_backwards_pumped(
        &self,
        dimension: usize,
        counter: VASSCounterIndex,
        limit: u32,
    ) -> bool {
        let mut visited = hashbrown::HashMap::new();
        let mut counters = VASSCounterValuation::zero(dimension);
        visited.insert(self.states[0].clone(), (1, counters.clone()));

        // iterate in reverse order
        for (update, state) in self.iter().rev() {
            // apply the reverse update since we are going backwards
            counters.apply_cfg_update(update.reverse());

            let entry = visited
                .entry(state.clone())
                .or_insert((0, VASSCounterValuation::zero(dimension)));

            // check that we have pumped and that we pumped the counter we care about
            if counters >= entry.1 && counters[counter] > entry.1[counter] {
                entry.0 += 1;
                entry.1 = counters.clone();
                if entry.0 > limit {
                    return true;
                }
            }
        }

        false
    }
}

impl<N: GIndex, L: Letter> Display for Path<N, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.start())?;
        for (letter, target) in self.iter() {
            write!(f, " --({:?})-> {:?}", letter, target)?;
        }
        Ok(())
    }
}

impl<N: GIndex> Path<N, CFGCounterUpdate> {
    pub fn to_compact_string(&self) -> String {
        self.transitions
            .iter()
            .map(|u| format!("{}", u))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl<N: GIndex, L: Letter> IntoIterator for Path<N, L> {
    type Item = (L, N);
    type IntoIter = std::iter::Zip<std::vec::IntoIter<L>, std::vec::IntoIter<N>>;

    fn into_iter(self) -> Self::IntoIter {
        let mut states = self.states;
        if !states.is_empty() {
            states.remove(0);
        }
        self.transitions.into_iter().zip(states)
    }
}
