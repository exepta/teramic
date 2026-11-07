use std::collections::BTreeMap;
use terra_core::{MAX_WORLD_DROPS, WorldDrop, kinetic::Cell};

pub type DropIndex = BTreeMap<Cell, Vec<(u64, [f64; 3])>>;
const SEPARATION: f64 = 0.26;

pub fn index_drops(drops: &[WorldDrop]) -> DropIndex {
    let mut result = DropIndex::new();
    for drop in drops.iter().take(MAX_WORLD_DROPS) {
        result
            .entry(cell(drop.position))
            .or_default()
            .push((drop.id, drop.position));
    }
    result
}

fn cell(point: [f64; 3]) -> Cell {
    point.map(|value| value.floor() as i64)
}

fn overlaps(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|axis| (a[axis] - b[axis]).abs() < SEPARATION)
}

fn nearby(
    index: &DropIndex,
    point: [f64; 3],
    mut visit: impl FnMut(u64, [f64; 3]) -> bool,
) -> bool {
    let low = point.map(|value| (value - SEPARATION).floor() as i64);
    let high = point.map(|value| (value + SEPARATION).floor() as i64);
    for x in low[0]..=high[0] {
        for y in low[1]..=high[1] {
            for z in low[2]..=high[2] {
                if index
                    .get(&[x, y, z])
                    .into_iter()
                    .flatten()
                    .any(|&(id, position)| !visit(id, position))
                {
                    return false;
                }
            }
        }
    }
    true
}

pub fn drop_space_clear(index: &DropIndex, point: [f64; 3], except: Option<u64>) -> bool {
    nearby(index, point, |id, other| {
        Some(id) == except || !overlaps(point, other)
    })
}

/// Old saved overlaps may separate, but cannot move farther into another item.
pub fn drop_step_clear(index: &DropIndex, from: [f64; 3], to: [f64; 3], id: u64) -> bool {
    nearby(index, to, |other_id, other| {
        if id == other_id || !overlaps(to, other) {
            return true;
        }
        let distance = |point: [f64; 3]| {
            (0..3)
                .map(|axis| (point[axis] - other[axis]).powi(2))
                .sum::<f64>()
        };
        overlaps(from, other) && distance(to) > distance(from) + 1e-12
    })
}

pub fn move_indexed_drop(index: &mut DropIndex, id: u64, from: [f64; 3], to: [f64; 3]) {
    remove_indexed_drop(index, id, from);
    index.entry(cell(to)).or_default().push((id, to));
}

pub fn remove_indexed_drop(index: &mut DropIndex, id: u64, from: [f64; 3]) {
    let old = cell(from);
    if let Some(entries) = index.get_mut(&old) {
        entries.retain(|(present, _)| *present != id);
        if entries.is_empty() {
            index.remove(&old);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_cell_boundaries_block_approach_but_old_overlaps_can_only_separate() {
        let mut index = DropIndex::from([([-1, 4, -1], vec![(1, [-0.01, 4.5, -0.01])])]);
        assert!(!drop_space_clear(&index, [0.01, 4.5, 0.01], None));
        assert!(drop_space_clear(&index, [0.01, 4.5, 0.01], Some(1)));
        assert!(!drop_step_clear(
            &index,
            [-0.4, 4.5, -0.01],
            [-0.2, 4.5, -0.01],
            2
        ));
        assert!(drop_step_clear(
            &index,
            [-0.1, 4.5, -0.01],
            [-0.2, 4.5, -0.01],
            2
        ));
        assert!(!drop_step_clear(
            &index,
            [-0.2, 4.5, -0.01],
            [-0.1, 4.5, -0.01],
            2
        ));
        move_indexed_drop(&mut index, 1, [-0.01, 4.5, -0.01], [1.01, 4.5, 0.01]);
        assert!(drop_space_clear(&index, [0.01, 4.5, 0.01], None));
        assert_eq!(index.values().map(Vec::len).sum::<usize>(), 1);
    }

    #[test]
    fn a_colocated_split_can_separate_but_its_rest_cannot_follow_into_it_or_bypass_a_queue() {
        let origin = [-0.01, 4.755, -0.01];
        let mut index = DropIndex::from([([-1, 4, -1], vec![(1, origin), (2, origin)])]);
        let next = [0.01, 4.755, -0.01];
        assert!(drop_step_clear(&index, origin, next, 2));
        move_indexed_drop(&mut index, 2, origin, next);
        assert!(!drop_step_clear(&index, origin, next, 1));
        assert!(!drop_step_clear(&index, next, origin, 2));
        assert!(drop_step_clear(&index, next, [0.03, 4.755, -0.01], 2));
        // Existing overlap recovery cannot grant passage into another, initially separated stack.
        index
            .entry([0, 4, -1])
            .or_default()
            .push((3, [0.29, 4.755, -0.01]));
        assert!(!drop_step_clear(&index, next, [0.05, 4.755, -0.01], 2));
        assert_eq!(
            index
                .values()
                .flatten()
                .map(|(id, _)| *id)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            3
        );
    }
}
