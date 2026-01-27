use petgraph::graph::NodeIndex;

use crate::automaton::{
    Alphabet, Automaton, Deterministic, InitializedAutomaton, Language, SingleFinalStateAutomaton,
    TransitionSystem,
    cfg::{
        CFG,
        update::{CFGCounterUpdatable, CFGCounterUpdate},
    },
    vass::counter::{VASSCounterIndex, VASSCounterValuation},
};

/// A CFG that tracks counters modulo given values.
/// This CFG is implemented without some underlying graph structure.
#[derive(Debug, Clone)]
pub struct ModuloCFG {
    mu: Vec<i32>,
    dimension: usize,
    alphabet: Vec<CFGCounterUpdate>,
    initial_valuation: VASSCounterValuation,
    final_valuation: VASSCounterValuation,
    initial_index: NodeIndex,
    final_index: NodeIndex,
}

impl ModuloCFG {
    pub fn new(
        mu: Vec<i32>,
        mut initial_valuation: VASSCounterValuation,
        mut final_valuation: VASSCounterValuation,
    ) -> Self {
        for &m in &mu {
            assert!(m > 0, "Modulo values must be positive");
        }

        let dimension = mu.len();
        assert_eq!(
            initial_valuation.dimension(),
            dimension,
            "Initial valuation dimension must match modulo dimension"
        );
        assert_eq!(
            final_valuation.dimension(),
            dimension,
            "Final valuation dimension must match modulo dimension"
        );

        initial_valuation.mod_euclid_slice_mut(&mu);
        final_valuation.mod_euclid_slice_mut(&mu);

        let mut cfg = ModuloCFG {
            mu,
            dimension,
            alphabet: CFGCounterUpdate::alphabet(dimension),
            initial_valuation,
            final_valuation,
            initial_index: NodeIndex::new(0), // to be set below
            final_index: NodeIndex::new(0),   // to be set below
        };

        // we precompute the initial and final indices to speed up operations later
        cfg.initial_index = cfg.counter_to_index(&cfg.initial_valuation);
        cfg.final_index = cfg.counter_to_index(&cfg.final_valuation);

        cfg
    }

    pub fn initial(
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
    ) -> Self {
        assert!(
            initial_valuation.dimension() == final_valuation.dimension(),
            "Initial and final valuations must have the same dimension"
        );

        Self::new(
            vec![2; initial_valuation.dimension()],
            initial_valuation,
            final_valuation,
        )
    }

    pub fn mu(&self) -> &[i32] {
        &self.mu
    }

    pub fn get_mu(&self, index: VASSCounterIndex) -> i32 {
        self.mu[index.to_usize()]
    }

    pub fn counter_to_index(&self, counter: &VASSCounterValuation) -> NodeIndex {
        assert_eq!(counter.dimension(), self.dimension);

        let mut index = 0;
        for (i, &val) in counter.iter().enumerate() {
            let mu = self.mu[i];

            assert!(val < mu, "Counter value {} exceeds modulo {}", val, mu);
            assert!(val >= 0, "Counter value {} is negative", val);

            index += val * self.mu[..i].iter().product::<i32>();
        }
        (index as u32).into()
    }

    pub fn index_to_counter(&self, index: NodeIndex) -> VASSCounterValuation {
        let mut counter = vec![0_i32; self.dimension];
        let mut remaining = index.index() as i32;
        for i in 0..self.dimension {
            let mu = self.mu[i];

            counter[i] = remaining % mu;
            remaining /= mu;
        }
        counter.into()
    }
}

impl Alphabet for ModuloCFG {
    type Letter = CFGCounterUpdate;

    fn alphabet(&self) -> &[Self::Letter] {
        &self.alphabet
    }
}

impl Automaton<Deterministic> for ModuloCFG {
    type NIndex = NodeIndex;

    type N = ();

    fn node_count(&self) -> usize {
        self.mu.iter().map(|&m| m as usize).product()
    }

    fn get_node(&self, _index: Self::NIndex) -> Option<&Self::N> {
        Some(&())
    }

    fn get_node_unchecked(&self, _index: Self::NIndex) -> &Self::N {
        &()
    }
}

impl TransitionSystem<Deterministic> for ModuloCFG {
    fn successor(&self, node: Self::NIndex, letter: &Self::Letter) -> Option<Self::NIndex> {
        let mut valuation = self.index_to_counter(node);
        valuation.apply_cfg_update_mod_slice(*letter, &self.mu);
        Some(self.counter_to_index(&valuation))
    }

    fn successors(&self, node: Self::NIndex) -> Box<dyn Iterator<Item = Self::NIndex> + '_> {
        let valuation = self.index_to_counter(node);

        Box::new(self.alphabet.iter().map(move |letter| {
            let mut new_valuation = valuation.clone();
            new_valuation.apply_cfg_update_mod_slice(*letter, &self.mu);
            self.counter_to_index(&new_valuation)
        }))
    }

    fn predecessors(&self, node: Self::NIndex) -> Box<dyn Iterator<Item = Self::NIndex> + '_> {
        let valuation = self.index_to_counter(node);

        Box::new(self.alphabet.iter().map(move |letter| {
            let mut new_valuation: VASSCounterValuation = valuation.clone();
            new_valuation.apply_cfg_update_mod_slice(letter.reverse(), &self.mu);
            self.counter_to_index(&new_valuation)
        }))
    }
}

impl InitializedAutomaton<Deterministic> for ModuloCFG {
    fn get_initial(&self) -> Self::NIndex {
        self.initial_index
    }

    fn is_accepting(&self, node: Self::NIndex) -> bool {
        self.final_index == node
    }
}

impl SingleFinalStateAutomaton<Deterministic> for ModuloCFG {
    fn get_final(&self) -> Self::NIndex {
        self.final_index
    }

    fn set_final(&mut self, node: Self::NIndex) {
        let counter = self.index_to_counter(node);
        self.final_valuation = counter;
        self.final_index = node;
    }
}

impl Language for ModuloCFG {
    fn accepts<'a>(&self, input: impl IntoIterator<Item = &'a Self::Letter>) -> bool
    where
        Self::Letter: 'a,
    {
        let mut current_index = self.get_initial();
        for letter in input {
            current_index = match self.successor(current_index, letter) {
                Some(succ) => succ,
                None => return false,
            };
        }
        self.is_accepting(current_index)
    }
}

impl CFG for ModuloCFG {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg_inc;

    #[test]
    fn test_counter_index_conversion() {
        let cfg = ModuloCFG::new(vec![5, 5, 5], vec![0; 3].into(), vec![0; 3].into());
        let counter = VASSCounterValuation::from(vec![2, 3, 4]);
        let index = cfg.counter_to_index(&counter);
        let converted_counter = cfg.index_to_counter(index);
        assert_eq!(counter, converted_counter);
    }

    #[test]
    fn test_index_counter_conversion() {
        let cfg = ModuloCFG::new(vec![3, 6], vec![0; 2].into(), vec![0; 2].into());
        let index = NodeIndex::new(14);
        let counter = cfg.index_to_counter(index);
        let converted_index = cfg.counter_to_index(&counter);
        assert_eq!(index, converted_index);
    }

    #[test]
    fn test_successor() {
        let cfg = ModuloCFG::new(vec![4, 4], vec![0; 2].into(), vec![0; 2].into());
        let node_index = cfg.counter_to_index(&VASSCounterValuation::from(vec![1, 3]));
        let update = cfg_inc!(1);
        let successor_index = cfg.successor(node_index, &update).unwrap();
        let successor_counter = cfg.index_to_counter(successor_index);
        assert_eq!(successor_counter, VASSCounterValuation::from(vec![1, 0]));
    }
}
