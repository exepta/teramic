use std::collections::{HashMap, VecDeque};
use terra_voxel::SectionPos;

/// Distance order for the first job, with an index for its vertical siblings.
/// Consumed siblings leave lazy order entries, discarded on the next pop.
#[derive(Default)]
pub(super) struct SectionQueue {
    order: VecDeque<SectionPos>,
    columns: HashMap<[i64; 2], Vec<SectionPos>>,
}

impl SectionQueue {
    pub fn push_back(&mut self, position: SectionPos) {
        let column = self.columns.entry([position.x, position.z]).or_default();
        if !column.contains(&position) {
            column.push(position);
            self.order.push_back(position);
        }
    }

    pub fn pop_front(&mut self) -> Option<SectionPos> {
        while let Some(position) = self.order.pop_front() {
            let coordinate = [position.x, position.z];
            let Some(column) = self.columns.get_mut(&coordinate) else {
                continue;
            };
            let Some(index) = column.iter().position(|p| *p == position) else {
                continue;
            };
            column.remove(index);
            if column.is_empty() {
                self.columns.remove(&coordinate);
                if self.columns.is_empty() {
                    self.order.clear();
                }
            }
            return Some(position);
        }
        None
    }

    pub fn take_column(
        &mut self,
        coordinate: [i64; 2],
        limit: usize,
        mut eligible: impl FnMut(SectionPos) -> bool,
    ) -> Vec<SectionPos> {
        let Some(column) = self.columns.get_mut(&coordinate) else {
            return Vec::new();
        };
        let mut taken = Vec::with_capacity(limit.min(column.len()));
        column.retain(|&position| {
            if taken.len() < limit && eligible(position) {
                taken.push(position);
                false
            } else {
                true
            }
        });
        if column.is_empty() {
            self.columns.remove(&coordinate);
            if self.columns.is_empty() {
                self.order.clear();
            }
        }
        taken
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.columns.values().map(Vec::len).sum()
    }
}

impl From<Vec<SectionPos>> for SectionQueue {
    fn from(positions: Vec<SectionPos>) -> Self {
        let mut queue = Self {
            order: VecDeque::with_capacity(positions.len()),
            ..Self::default()
        };
        for position in positions {
            queue.push_back(position);
        }
        queue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_batch_only_visits_its_siblings_and_preserves_global_priority() {
        let positions: Vec<_> = (-32..=32)
            .flat_map(|x| {
                (-32..=32).flat_map(move |z| (-3..=3).map(move |y| SectionPos::new(x, y, z)))
            })
            .collect();
        let mut queue: SectionQueue = positions.clone().into();
        let first = queue.pop_front().unwrap();
        assert_eq!(first, positions[0]);
        let mut inspected = 0;
        let batch = queue.take_column([first.x, first.z], 15, |_| {
            inspected += 1;
            true
        });
        assert_eq!(inspected, 6);
        assert_eq!(batch, positions[1..7]);
        assert_eq!(queue.len(), positions.len() - 7);
        assert_eq!(queue.pop_front(), Some(positions[7]));
        assert_eq!(queue.len(), positions.len() - 8);
    }

    #[test]
    fn limits_pending_siblings_duplicates_and_eviction_do_not_leave_live_entries() {
        let points: Vec<_> = (-12..=12).map(|y| SectionPos::new(-1, y, -2)).collect();
        let mut queue: SectionQueue = points.clone().into();
        queue.push_back(points[0]);
        assert_eq!(queue.len(), points.len());
        let batch = queue.take_column([-1, -2], 3, |p| p.y % 2 == 0);
        assert_eq!(batch, [points[0], points[2], points[4]]);
        assert_eq!(queue.pop_front(), Some(points[1]));
        while queue.pop_front().is_some() {}
        assert!(queue.is_empty());
        assert!(queue.order.is_empty());
        assert!(queue.columns.is_empty());
        queue.push_back(points[0]);
        assert_eq!(queue.take_column([-1, -2], 1, |_| true), [points[0]]);
        assert!(queue.order.is_empty());
        assert!(queue.pop_front().is_none());
        queue = vec![points[1]].into();
        assert_eq!(queue.pop_front(), Some(points[1]));
    }
}
