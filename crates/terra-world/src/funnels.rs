//! A funnel mouth holds its visible stack until the existing transfer accepts it.
use crate::{edits::EditJournal, streaming::ChunkStore};
use terra_core::{WorldDrop, logistics};
mod support;
pub use support::supported_with;

type Cell = [i64; 3];

pub const INLET_TOP: f64 = 0.5;
const MOUTH_RADIUS: f64 = 0.32;

fn funnel(position: Cell, journal: &EditJournal) -> bool {
    journal.block(position).is_some_and(|material| {
        logistics::is_funnel(material)
            && journal
                .object_state(position)
                .is_some_and(|state| state.valid_for(position, material))
    })
}

pub fn loaded(position: Cell, chunks: &ChunkStore, journal: &EditJournal) -> bool {
    funnel(position, journal) && chunks.block_at(position) == journal.block(position)
}

/// A resting mouth stack remains supported while its destination or section is unloaded.
pub fn carrying(drop: &WorldDrop, journal: &EditJournal) -> Option<Cell> {
    if !journal.has_logistics()
        || drop
            .position
            .iter()
            .any(|value| !value.is_finite() || value.abs() > 1_000_001.0)
        || drop.velocity[1] > 0.5
    {
        return None;
    }
    let axes = bevy_math::Mat3::from_quat(bevy_math::Quat::from_array(drop.rotation));
    let half_y =
        f64::from((axes.x_axis.y.abs() + axes.y_axis.y.abs() + axes.z_axis.y.abs()) * 0.12);
    let [x, y, z] = drop.position.map(|value| value.floor() as i64);
    for dy in [0, -1, -2] {
        let cell = [x, y + dy, z];
        let gap = drop.position[1] - cell[1] as f64 - INLET_TOP - half_y;
        if (-0.035..=0.08).contains(&gap)
            && inside_mouth(drop.position, cell)
            && funnel(cell, journal)
        {
            return Some(cell);
        }
    }
    None
}

/// Test each downward physics substep; outlet stacks already below the plane never re-enter.
pub fn crossed_inlet(
    from: [f64; 3],
    to: [f64; 3],
    half_y: f64,
    journal: &EditJournal,
) -> Option<Cell> {
    if !journal.has_logistics()
        || from
            .iter()
            .chain(to.iter())
            .any(|value| !value.is_finite() || value.abs() > 1_000_001.0)
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
        let point = std::array::from_fn(|axis| from[axis] + (to[axis] - from[axis]) * t);
        let cell = [point[0].floor() as i64, y, point[2].floor() as i64];
        if inside_mouth(point, cell) && funnel(cell, journal) {
            return Some(cell);
        }
    }
    None
}

pub fn stabilize(drop: &mut WorldDrop, position: Cell) {
    drop.position[1] = position[1] as f64 + INLET_TOP + 0.13;
    drop.velocity = [0.0; 3];
    drop.angular_velocity = [0.0; 3];
    drop.rotation = [0.0, 0.0, 0.0, 1.0];
    drop.grounded = true;
}

fn inside_mouth(point: [f64; 3], position: Cell) -> bool {
    (point[0] - position[0] as f64 - 0.5).abs() <= MOUTH_RADIUS
        && (point[2] - position[2] as f64 - 0.5).abs() <= MOUTH_RADIUS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edits::BlockObjectState;
    use terra_core::{GameplayState, ItemKind, ItemStack, beds::DyeColor};

    #[test]
    fn mouths_hold_only_real_funnels_and_do_not_recapture_their_lower_outlets() {
        for material in [logistics::FUNNEL, logistics::FILTERED_FUNNEL] {
            for facing in 0..8 {
                let mut journal = EditJournal::default();
                let position = [-16, -1, -16];
                journal.record(position, material).unwrap();
                journal
                    .set_object_state(
                        position,
                        Some(BlockObjectState {
                            facing,
                            ..Default::default()
                        }),
                    )
                    .unwrap();
                let mut game = GameplayState::default();
                assert!(game.drop_stack(
                    [-15.5, -0.37, -15.5],
                    ItemStack {
                        item: ItemKind::Dye(DyeColor::Rgb([71, 93, 184])),
                        count: 11,
                    },
                    [0.0; 3],
                    0.0,
                ));
                let mut drop = game.drops[0].clone();
                drop.rotation = [0.0, 0.0, 0.0, 1.0];
                assert_eq!(carrying(&drop, &journal), Some(position));
                stabilize(&mut drop, position);
                assert_eq!(carrying(&drop, &journal), Some(position));
                assert_eq!(
                    crossed_inlet([-15.5, 0.0, -15.5], [-15.5, -0.5, -15.5], 0.12, &journal),
                    Some(position)
                );
                drop.position[1] = position[1] as f64 - 0.16;
                assert_eq!(carrying(&drop, &journal), None);
                let mut below = drop.position;
                below[1] -= 0.2;
                assert_eq!(crossed_inlet(drop.position, below, 0.12, &journal), None);
                journal.record(position, 0).unwrap();
                stabilize(&mut drop, position);
                assert_eq!(carrying(&drop, &journal), None);
            }
        }
    }
}
