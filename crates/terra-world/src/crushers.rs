//! The inlet is physical support even when its input is full or has no recipe.
use crate::edits::EditJournal;
#[cfg(feature = "bevy")]
use crate::streaming::ChunkStore;
use terra_core::WorldDrop;

type Cell = [i64; 3];
pub const INLET_TOP: f64 = 1.0;

#[cfg(feature = "bevy")]
pub fn loaded(position: Cell, chunks: &ChunkStore, journal: &EditJournal) -> bool {
    journal.valid_crusher(position)
        && chunks.block_at(position) == Some(99)
        && journal.object_state(position).is_some_and(|state| {
            state
                .peer
                .is_none_or(|peer| chunks.block_at(peer) == Some(99))
        })
}

/// Metadata-only support keeps an actual resting stack safe across section unloads.
pub fn carrying(drop: &WorldDrop, journal: &EditJournal) -> Option<Cell> {
    if !journal.has_kinetics()
        || drop.position.iter().any(|v| !v.is_finite())
        || drop.velocity[1] > 0.5
    {
        return None;
    }
    let x = drop.position[0].floor() as i64;
    let z = drop.position[2].floor() as i64;
    let y = drop.position[1].floor() as i64;
    for dy in [0, -1, -2] {
        let cell = [x, y + dy, z];
        let Some(primary) = journal.crusher_primary(cell) else {
            continue;
        };
        if (drop.position[0] - cell[0] as f64 - 0.5).abs() > 0.32
            || (drop.position[2] - cell[2] as f64 - 0.5).abs() > 0.32
        {
            continue;
        }
        let axes = bevy_math::Mat3::from_quat(bevy_math::Quat::from_array(drop.rotation));
        let half_y =
            f64::from((axes.x_axis.y.abs() + axes.y_axis.y.abs() + axes.z_axis.y.abs()) * 0.12);
        let gap = drop.position[1] - cell[1] as f64 - INLET_TOP - half_y;
        if (-0.035..=0.08).contains(&gap) {
            return Some(primary);
        }
    }
    None
}

/// Physics calls this on each downward substep so an open inlet cannot be tunneled through.
pub fn crossed_inlet(
    from: [f64; 3],
    to: [f64; 3],
    half_y: f64,
    journal: &EditJournal,
) -> Option<Cell> {
    if !journal.has_kinetics()
        || from.iter().chain(to.iter()).any(|v| !v.is_finite())
        || !half_y.is_finite()
        || half_y <= 0.0
        || to[1] >= from[1]
    {
        return None;
    }
    let base = from[1].floor() as i64;
    for y in [base, base - 1, base - 2] {
        let height = y as f64 + INLET_TOP + half_y;
        if from[1] < height || to[1] > height {
            continue;
        }
        let t = (from[1] - height) / (from[1] - to[1]);
        let point: [f64; 3] = std::array::from_fn(|i| from[i] + (to[i] - from[i]) * t);
        let cell = [point[0].floor() as i64, y, point[2].floor() as i64];
        if (point[0] - cell[0] as f64 - 0.5).abs() <= 0.32
            && (point[2] - cell[2] as f64 - 0.5).abs() <= 0.32
            && let Some(primary) = journal.crusher_primary(cell)
        {
            return Some(primary);
        }
    }
    None
}

pub fn stabilize(drop: &mut WorldDrop, primary: Cell, journal: &EditJournal) {
    let cell = journal
        .object_state(primary)
        .and_then(|state| state.peer)
        .filter(|peer| {
            (drop.position[0] - peer[0] as f64 - 0.5).abs() <= 0.32
                && (drop.position[2] - peer[2] as f64 - 0.5).abs() <= 0.32
        })
        .unwrap_or(primary);
    drop.position[1] = cell[1] as f64 + INLET_TOP + 0.13;
    drop.velocity = [0.0; 3];
    drop.angular_velocity = [0.0; 3];
    drop.rotation = [0.0, 0.0, 0.0, 1.0];
    drop.grounded = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edits::BlockObjectState;
    use terra_core::{ItemKind, ItemStack};

    #[test]
    fn single_cell_and_legacy_inlets_replay_without_requiring_loaded_chunks() {
        let mut journal = EditJournal::default();
        for (p, state) in [
            ([0, 0, 0], BlockObjectState::default()),
            (
                [15, 0, 0],
                BlockObjectState {
                    peer: Some([16, 0, 0]),
                    ..Default::default()
                },
            ),
            (
                [16, 0, 0],
                BlockObjectState {
                    peer: Some([15, 0, 0]),
                    secondary: true,
                    ..Default::default()
                },
            ),
        ] {
            journal.record(p, 99).unwrap();
            journal.set_object_state(p, Some(state)).unwrap();
        }
        for (x, owner) in [(0.5, [0, 0, 0]), (15.5, [15, 0, 0]), (16.5, [15, 0, 0])] {
            let mut game = terra_core::GameplayState::default();
            assert!(game.drop_stack(
                [x, 1.13, 0.5],
                ItemStack {
                    item: ItemKind::Block(3),
                    count: 2
                },
                [0.0; 3],
                0.0
            ));
            assert_eq!(carrying(&game.drops[0], &journal), Some(owner));
            game.drops[0].position[1] += 0.2;
            assert_eq!(carrying(&game.drops[0], &journal), None);
        }
        assert_eq!(journal.crusher_crank_position([0, 0, 0]), Some([1, 0, 0]));
        assert_eq!(journal.crusher_crank_position([15, 0, 0]), Some([17, 0, 0]));
    }
}
