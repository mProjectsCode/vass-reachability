use hashbrown::HashSet;
use itertools::Itertools;

use super::{ImplicitCFGProduct, state::MultiGraphState};
use crate::automaton::{
    Alphabet, Automaton, AutomatonEdge, Deterministic, ExplicitEdgeAutomaton, InitializedAutomaton,
    TransitionSystem, cfg::update::CFGCounterUpdate, path::Path,
};

#[derive(Debug, Clone)]
pub struct ImplicitCFGProductView<'p> {
    pub product: &'p ImplicitCFGProduct,
    active_cfg_indices: Box<[usize]>,
}

impl<'p> ImplicitCFGProductView<'p> {
    pub fn full(product: &'p ImplicitCFGProduct) -> Self {
        Self::from_indices(product, 0..product.cfgs.len())
    }

    pub fn from_indices(
        product: &'p ImplicitCFGProduct,
        active_cfg_indices: impl IntoIterator<Item = usize>,
    ) -> Self {
        let active_cfg_indices = active_cfg_indices.into_iter().collect_vec();
        assert!(
            !active_cfg_indices.is_empty(),
            "Product view must contain at least one CFG"
        );

        let mut seen = HashSet::new();
        for index in &active_cfg_indices {
            assert!(
                *index < product.cfgs.len(),
                "Product view CFG index {} is out of bounds for {} CFGs",
                index,
                product.cfgs.len()
            );
            assert!(
                seen.insert(*index),
                "Product view CFG index {} occurs more than once",
                index
            );
        }

        Self {
            product,
            active_cfg_indices: active_cfg_indices.into_boxed_slice(),
        }
    }

    pub fn without_indices(
        product: &'p ImplicitCFGProduct,
        excluded_cfg_indices: impl IntoIterator<Item = usize>,
    ) -> Self {
        let excluded = excluded_cfg_indices.into_iter().collect::<HashSet<_>>();
        Self::from_indices(
            product,
            (0..product.cfgs.len()).filter(|index| !excluded.contains(index)),
        )
    }

    pub fn active_cfg_indices(&self) -> &[usize] {
        &self.active_cfg_indices
    }

    pub fn dimension(&self) -> usize {
        self.product.dimension
    }

    pub fn project_state(&self, state: &MultiGraphState) -> MultiGraphState {
        MultiGraphState::from(
            self.active_cfg_indices
                .iter()
                .map(|index| state[*index])
                .collect_vec()
                .into_boxed_slice(),
        )
    }

    pub fn project_path(
        &self,
        path: &Path<MultiGraphState, CFGCounterUpdate>,
    ) -> Path<MultiGraphState, CFGCounterUpdate> {
        let mut projected = Path::new(self.project_state(path.start()));

        for (letter, state) in path.iter() {
            projected.add(*letter, self.project_state(state));
        }

        projected
    }

    fn view_successor(
        &self,
        node: &MultiGraphState,
        letter: &CFGCounterUpdate,
    ) -> Option<MultiGraphState> {
        let mut new_states = Vec::with_capacity(self.active_cfg_indices.len());

        for (view_index, product_index) in self.active_cfg_indices.iter().enumerate() {
            let cfg = &self.product.cfgs[*product_index];
            let target = cfg.successor(&node[view_index], letter)?;
            new_states.push(target);
        }

        Some(MultiGraphState::from(new_states.into_boxed_slice()))
    }
}

impl Alphabet for ImplicitCFGProductView<'_> {
    type Letter = CFGCounterUpdate;

    fn alphabet(&self) -> &[Self::Letter] {
        self.product.alphabet()
    }
}

impl Automaton<Deterministic> for ImplicitCFGProductView<'_> {
    type NIndex = MultiGraphState;
    type N = ();

    fn node_count(&self) -> usize {
        self.active_cfg_indices
            .iter()
            .map(|index| self.product.cfgs[*index].node_count())
            .product()
    }

    fn get_node(&self, _index: &Self::NIndex) -> Option<&Self::N> {
        Some(&())
    }

    fn get_node_unchecked(&self, _index: &Self::NIndex) -> &Self::N {
        &()
    }
}

impl TransitionSystem<Deterministic> for ImplicitCFGProductView<'_> {
    fn successor(&self, node: &Self::NIndex, letter: &Self::Letter) -> Option<Self::NIndex> {
        self.view_successor(node, letter)
    }

    fn successors<'a>(
        &'a self,
        node: &Self::NIndex,
    ) -> Box<dyn Iterator<Item = Self::NIndex> + 'a> {
        let node = node.clone();

        Box::new(
            self.alphabet()
                .iter()
                .filter_map(move |letter| self.successor(&node, letter)),
        )
    }

    fn predecessors<'a>(
        &'a self,
        node: &Self::NIndex,
    ) -> Box<dyn Iterator<Item = Self::NIndex> + 'a> {
        let mut result = HashSet::new();

        for letter in self.alphabet() {
            let mut per_comp_preds = Vec::with_capacity(self.active_cfg_indices.len());
            let mut empty = false;

            for (view_index, product_index) in self.active_cfg_indices.iter().enumerate() {
                let cfg = &self.product.cfgs[*product_index];
                let target = node[view_index];
                let mut preds = cfg
                    .incoming_edge_indices(&target)
                    .filter(|edge| cfg.get_edge_unchecked(edge).matches(letter))
                    .map(|edge| cfg.edge_source_unchecked(&edge))
                    .collect_vec();
                preds.sort_unstable();
                preds.dedup();

                if preds.is_empty() {
                    empty = true;
                    break;
                }

                per_comp_preds.push(preds);
            }

            if empty {
                continue;
            }

            for combo in per_comp_preds.into_iter().multi_cartesian_product() {
                result.insert(MultiGraphState::from(combo.into_boxed_slice()));
            }
        }

        let mut result = result.into_iter().collect_vec();
        result.sort_unstable();
        Box::new(result.into_iter())
    }
}

impl InitializedAutomaton<Deterministic> for ImplicitCFGProductView<'_> {
    fn get_initial(&self) -> Self::NIndex {
        MultiGraphState::from(
            self.active_cfg_indices
                .iter()
                .map(|index| self.product.cfgs[*index].get_initial())
                .collect_vec()
                .into_boxed_slice(),
        )
    }

    fn is_accepting(&self, node: &Self::NIndex) -> bool {
        self.active_cfg_indices
            .iter()
            .enumerate()
            .all(|(view_index, product_index)| {
                self.product.cfgs[*product_index].is_accepting(&node[view_index])
            })
    }
}
