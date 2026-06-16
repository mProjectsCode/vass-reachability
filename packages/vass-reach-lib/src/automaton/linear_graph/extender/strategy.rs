use hashbrown::HashSet;

use crate::config::LinearGraphInterpolationStrategy;

pub(super) trait InterpolationStrategy {
    fn next_batch(&mut self, pending: &[usize]) -> Vec<usize>;
    fn on_unreachable(&mut self, pending: &mut Vec<usize>, batch: &[usize]);
    fn on_reachable(
        &mut self,
        pending: &mut Vec<usize>,
        batch: &[usize],
        used_in_batch: &HashSet<usize>,
    );
    fn on_unknown(&mut self, pending: &mut Vec<usize>, batch: &[usize]);
}

pub(super) fn interpolation_strategy(
    strategy: LinearGraphInterpolationStrategy,
    pending_len: usize,
) -> Box<dyn InterpolationStrategy> {
    match strategy {
        LinearGraphInterpolationStrategy::AdaptiveBatch => {
            Box::new(AdaptiveBatchStrategy::new(pending_len))
        }
        LinearGraphInterpolationStrategy::Linear => Box::new(LinearInterpolationStrategy),
    }
}

struct AdaptiveBatchStrategy {
    batch_size: usize,
}

impl AdaptiveBatchStrategy {
    fn new(pending_len: usize) -> Self {
        Self {
            batch_size: next_halving_batch_size(pending_len),
        }
    }
}

impl InterpolationStrategy for AdaptiveBatchStrategy {
    fn next_batch(&mut self, pending: &[usize]) -> Vec<usize> {
        let batch_len = self.batch_size.min(pending.len()).max(1);
        pending[..batch_len].to_vec()
    }

    fn on_unreachable(&mut self, pending: &mut Vec<usize>, batch: &[usize]) {
        remove_batch(pending, batch);
        self.batch_size = next_halving_batch_size(pending.len());
    }

    fn on_reachable(
        &mut self,
        pending: &mut Vec<usize>,
        batch: &[usize],
        used_in_batch: &HashSet<usize>,
    ) {
        if used_in_batch.is_empty() {
            if batch.len() == 1 {
                remove_batch(pending, batch);
            } else {
                self.batch_size = next_halving_batch_size(batch.len());
            }
        } else {
            pending.retain(|region| !used_in_batch.contains(region));
            self.batch_size = next_halving_batch_size(pending.len());
        }
    }

    fn on_unknown(&mut self, pending: &mut Vec<usize>, batch: &[usize]) {
        if batch.len() == 1 {
            remove_batch(pending, batch);
        } else {
            self.batch_size = next_halving_batch_size(batch.len());
        }
    }
}

struct LinearInterpolationStrategy;

impl InterpolationStrategy for LinearInterpolationStrategy {
    fn next_batch(&mut self, pending: &[usize]) -> Vec<usize> {
        pending.first().copied().into_iter().collect()
    }

    fn on_unreachable(&mut self, pending: &mut Vec<usize>, batch: &[usize]) {
        remove_batch(pending, batch);
    }

    fn on_reachable(
        &mut self,
        pending: &mut Vec<usize>,
        batch: &[usize],
        used_in_batch: &HashSet<usize>,
    ) {
        if used_in_batch.is_empty() {
            remove_batch(pending, batch);
        } else {
            pending.retain(|region| !used_in_batch.contains(region));
        }
    }

    fn on_unknown(&mut self, pending: &mut Vec<usize>, batch: &[usize]) {
        remove_batch(pending, batch);
    }
}

fn remove_batch(pending: &mut Vec<usize>, batch: &[usize]) {
    pending.retain(|region| !batch.contains(region));
}

fn next_halving_batch_size(len: usize) -> usize {
    len.div_ceil(2).max(1)
}
