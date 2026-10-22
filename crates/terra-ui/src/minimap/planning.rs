use std::collections::{BinaryHeap, HashSet};

const LIVE_LIMIT: usize = 4;
pub(super) const WORK_LIMIT: usize = 128;
type Priority = (bool, i64, [i64; 2]);

fn probe_visible_grid(entries: usize, low: [i64; 2], high: [i64; 2]) -> bool {
    let extent = |axis| (i128::from(high[axis]) - i128::from(low[axis]) + 1).max(0);
    extent(0).saturating_mul(extent(1)) < entries as i128
}

pub(super) fn visible_catalog(
    catalog: &HashSet<[i64; 2]>,
    low: [i64; 2],
    high: [i64; 2],
) -> impl Iterator<Item = [i64; 2]> {
    let grid = probe_visible_grid(catalog.len(), low, high);
    let visible = grid.then(|| {
        (low[0]..=high[0])
            .flat_map(move |x| (low[1]..=high[1]).map(move |z| [x, z]))
            .filter(|p| catalog.contains(p))
    });
    let sparse = (!grid).then(|| {
        catalog
            .iter()
            .copied()
            .filter(move |p| (0..2).all(|axis| p[axis] >= low[axis] && p[axis] <= high[axis]))
    });
    visible
        .into_iter()
        .flatten()
        .chain(sparse.into_iter().flatten())
}

#[derive(Default)]
pub(super) struct Candidates {
    live: BinaryHeap<Priority>,
    disk: BinaryHeap<Priority>,
}

impl Candidates {
    pub fn push(&mut self, coordinate: [i64; 2], live: bool, previously_live: bool, distance: i64) {
        let (heap, limit) = if live {
            (&mut self.live, LIVE_LIMIT)
        } else {
            (&mut self.disk, WORK_LIMIT)
        };
        let priority = (previously_live, distance, coordinate);
        if heap.len() < limit {
            heap.push(priority);
        } else if heap.peek().is_some_and(|worst| priority < *worst) {
            *heap.peek_mut().expect("bounded nonempty candidate heap") = priority;
        }
    }

    pub fn ordered(self) -> impl Iterator<Item = [i64; 2]> {
        self.live
            .into_sorted_vec()
            .into_iter()
            .chain(self.disk.into_sorted_vec())
            .take(WORK_LIMIT)
            .map(|(_, _, coordinate)| coordinate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_histories_probe_only_the_visible_rectangle_but_sparse_wide_maps_scan_entries() {
        let catalog: HashSet<_> = (-10_000..10_000).map(|x| [x, -2]).collect();
        let low = [-3, -3];
        let high = [3, 3];
        assert!(probe_visible_grid(catalog.len(), low, high));
        assert_eq!(
            visible_catalog(&catalog, low, high).collect::<HashSet<_>>(),
            (-3..=3).map(|x| [x, -2]).collect()
        );
        assert!(!probe_visible_grid(
            catalog.len(),
            [i64::MIN; 2],
            [i64::MAX; 2]
        ));
        assert_eq!(
            visible_catalog(&catalog, [i64::MIN; 2], [i64::MAX; 2]).count(),
            catalog.len()
        );
        assert_eq!(visible_catalog(&catalog, [4, 4], [3, 3]).count(), 0);
    }

    #[test]
    fn bounded_plan_matches_full_sort_with_independent_live_and_io_budgets() {
        let mut candidates = Candidates::default();
        let mut reference = Vec::new();
        for x in (-10_000_i64..10_000).rev() {
            let live = x % 3 == 0;
            let old = x % 7 == 0;
            let distance = x * x;
            candidates.push([x, -x], live, old, distance);
            reference.push((!live, old, distance, [x, -x]));
            assert!(candidates.live.len() <= LIVE_LIMIT);
            assert!(candidates.disk.len() <= WORK_LIMIT);
        }
        reference.sort_unstable();
        let mut live = 0;
        let expected: Vec<_> = reference
            .into_iter()
            .filter(|(disk, ..)| {
                if *disk {
                    true
                } else {
                    live += 1;
                    live <= LIVE_LIMIT
                }
            })
            .take(WORK_LIMIT)
            .map(|(_, _, _, position)| position)
            .collect();
        assert_eq!(candidates.ordered().collect::<Vec<_>>(), expected);
    }

    #[test]
    fn all_live_work_keeps_four_snapshots_and_does_not_revisit_dirty_tiles_as_io() {
        let mut candidates = Candidates::default();
        for x in 0..4225_i64 {
            candidates.push([-x, 0], true, false, x * x);
        }
        assert_eq!(
            candidates.ordered().collect::<Vec<_>>(),
            [[0, 0], [-1, 0], [-2, 0], [-3, 0]]
        );
    }
}
