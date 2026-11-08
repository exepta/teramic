//! Optional build guides share the authoritative train-part identities and cell grid.
use crate::{train_parts, trains::VehicleKind};

pub const MIN_WHEELBASE: u8 = 3;
pub const MAX_WHEELBASE: u8 = 15;
pub const DEFAULT_WHEELBASE: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlueprintCell {
    pub cell: [i64; 3],
    pub material: u16,
    pub facing: u8,
}

/// A purely visual guide. It never reserves cells, consumes items or edits the world.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
pub struct TrainBlueprint {
    pub station: Option<[i64; 3]>,
    pub kind: VehicleKind,
    pub cells: Vec<BlueprintCell>,
}

/// Build the default connected vehicle around its chassis.
pub fn template(chassis: [i64; 3], facing: u8, kind: VehicleKind) -> Vec<BlueprintCell> {
    template_with_wheelbase(chassis, facing, kind, DEFAULT_WHEELBASE)
}

/// Wheelbase is the center-to-center distance between the two rail wheelsets.
pub fn template_with_wheelbase(
    chassis: [i64; 3],
    facing: u8,
    kind: VehicleKind,
    wheelbase: u8,
) -> Vec<BlueprintCell> {
    if facing >= 4 || !(MIN_WHEELBASE..=MAX_WHEELBASE).contains(&wheelbase) {
        return Vec::new();
    }
    let length = i64::from(wheelbase);
    let mut local = (-2..=length)
        .map(|z| {
            (
                [0, 0, z],
                if z == 0 {
                    train_parts::CHASSIS
                } else {
                    train_parts::FRAME
                },
            )
        })
        .collect::<Vec<_>>();
    local.extend([
        ([0, -1, -1], train_parts::BOGIE),
        ([0, -1, length - 1], train_parts::BOGIE),
        ([0, 0, -3], train_parts::COUPLER),
        ([0, 0, length + 1], train_parts::COUPLER),
        (
            [0, 1, 1],
            match kind {
                VehicleKind::Locomotive => train_parts::CAB,
                VehicleKind::Passenger => train_parts::SEAT,
                VehicleKind::Tender | VehicleKind::Freight => train_parts::CARGO,
            },
        ),
    ]);
    if kind == VehicleKind::Locomotive {
        local.push(([0, 1, 2], train_parts::ENGINE));
    }
    local
        .into_iter()
        .map(|([x, y, z], material)| {
            let [x, z] = match facing {
                0 => [x, z],
                1 => [z, -x],
                2 => [-x, -z],
                _ => [-z, x],
            };
            BlueprintCell {
                cell: [chassis[0] + x, chassis[1] + y, chassis[2] + z],
                material,
                facing: if material == train_parts::CAB {
                    (facing + 2) % 4
                } else {
                    facing
                },
            }
        })
        .collect()
}

/// Green ghosts disappear only when the correct part and required axis are present.
pub fn matches(cell: BlueprintCell, material: Option<u16>, facing: u8) -> bool {
    material == Some(cell.material)
        && match cell.material {
            train_parts::BOGIE | train_parts::CHASSIS | train_parts::COUPLER => {
                facing % 2 == cell.facing % 2
            }
            train_parts::CAB | train_parts::SEAT => facing == cell.facing,
            _ => true,
        }
}

/// Aim feedback intersects only the visual cell; this does not make ghosts solid.
pub fn target(
    cells: &[BlueprintCell],
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
) -> Option<BlueprintCell> {
    if !origin
        .iter()
        .chain(direction.iter())
        .all(|value| value.is_finite())
        || !reach.is_finite()
        || reach <= 0.0
    {
        return None;
    }
    cells
        .iter()
        .filter_map(|part| {
            let mut near = 0.0_f64;
            let mut far = reach;
            for axis in 0..3 {
                let low = part.cell[axis] as f64;
                let high = low + 1.0;
                if direction[axis].abs() < 1e-8 {
                    if origin[axis] < low || origin[axis] > high {
                        return None;
                    }
                } else {
                    let a = (low - origin[axis]) / direction[axis];
                    let b = (high - origin[axis]) / direction[axis];
                    near = near.max(a.min(b));
                    far = far.min(a.max(b));
                    if near > far {
                        return None;
                    }
                }
            }
            Some((near, *part))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, part)| part)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn all_four_guides_rotate_the_complete_connected_fixture_without_duplicate_cells() {
        for kind in [
            VehicleKind::Locomotive,
            VehicleKind::Tender,
            VehicleKind::Passenger,
            VehicleKind::Freight,
        ] {
            for facing in 0..4 {
                let parts = template([16, 18, -17], facing, kind);
                assert_eq!(
                    parts.len(),
                    if kind == VehicleKind::Locomotive {
                        13
                    } else {
                        12
                    }
                );
                assert_eq!(
                    parts
                        .iter()
                        .filter(|part| part.material == train_parts::FRAME)
                        .count(),
                    6
                );
                assert_eq!(
                    parts
                        .iter()
                        .filter(|part| part.material == train_parts::BOGIE)
                        .count(),
                    2
                );
                assert_eq!(
                    parts
                        .iter()
                        .map(|part| part.cell)
                        .collect::<BTreeSet<_>>()
                        .len(),
                    parts.len()
                );
                assert!(parts.iter().all(|part| part.facing
                    == if part.material == train_parts::CAB {
                        (facing + 2) % 4
                    } else {
                        facing
                    }));
                let chassis = parts
                    .iter()
                    .find(|part| part.material == train_parts::CHASSIS)
                    .unwrap();
                assert_eq!(chassis.cell, [16, 18, -17]);
                for bogie in parts
                    .iter()
                    .filter(|part| part.material == train_parts::BOGIE)
                {
                    let above = [bogie.cell[0], bogie.cell[1] + 1, bogie.cell[2]];
                    assert!(
                        parts
                            .iter()
                            .any(|part| part.cell == above && part.material == train_parts::FRAME)
                    );
                }
            }
        }
        assert!(template([0; 3], 4, VehicleKind::Locomotive).is_empty());
    }

    #[test]
    fn aim_reports_the_nearest_part_within_the_visible_reach() {
        let parts = template([0, 2, 0], 0, VehicleKind::Locomotive);
        let hit = target(&parts, [-3.0, 2.5, 0.5], [1.0, 0.0, 0.0], 5.0).unwrap();
        assert_eq!(hit.material, train_parts::CHASSIS);
        assert!(target(&parts, [-3.0, 2.5, 0.5], [1.0, 0.0, 0.0], 2.0).is_none());
        assert!(target(&parts, [f64::NAN; 3], [1.0, 0.0, 0.0], 5.0).is_none());
    }

    #[test]
    fn powered_guides_keep_the_frame_connected_at_every_supported_wheelbase_without_fixed_cargo() {
        for wheelbase in MIN_WHEELBASE..=MAX_WHEELBASE {
            for facing in 0..4 {
                let cells =
                    template_with_wheelbase([0, 2, 0], facing, VehicleKind::Locomotive, wheelbase);
                assert_eq!(cells.len(), usize::from(wheelbase) + 9);
                assert!(
                    cells
                        .iter()
                        .any(|part| part.material == train_parts::ENGINE)
                );
                assert!(!cells.iter().any(|part| part.material == train_parts::CARGO));
                let bogies = cells
                    .iter()
                    .filter(|part| part.material == train_parts::BOGIE)
                    .collect::<Vec<_>>();
                assert_eq!(bogies.len(), 2);
                assert_eq!(
                    (0..3)
                        .map(|axis| bogies[0].cell[axis].abs_diff(bogies[1].cell[axis]))
                        .sum::<u64>(),
                    u64::from(wheelbase),
                );
                let occupied: BTreeSet<_> = cells.iter().map(|part| part.cell).collect();
                let mut connected = BTreeSet::new();
                let mut pending = vec![cells[0].cell];
                while let Some(cell) = pending.pop() {
                    if !connected.insert(cell) {
                        continue;
                    }
                    for axis in 0..3 {
                        for step in [-1, 1] {
                            let mut neighbor = cell;
                            neighbor[axis] += step;
                            if occupied.contains(&neighbor) && !connected.contains(&neighbor) {
                                pending.push(neighbor);
                            }
                        }
                    }
                }
                assert_eq!(connected, occupied, "every guide part touches its body");
            }
        }
        for wheelbase in [0, 2, 16, 255] {
            assert!(
                template_with_wheelbase([0; 3], 0, VehicleKind::Locomotive, wheelbase).is_empty()
            );
        }
    }
}
