//! Optional build guides share the authoritative train-part identities and cell grid.
use crate::{train_parts, trains::VehicleKind};

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

/// Build a seven-cell vehicle around its chassis, rotating every cell together.
pub fn template(chassis: [i64; 3], facing: u8, kind: VehicleKind) -> Vec<BlueprintCell> {
    if facing >= 4 {
        return Vec::new();
    }
    let mut local = (-2..=4)
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
        ([0, -1, 3], train_parts::BOGIE),
        ([0, 0, -3], train_parts::COUPLER),
        ([0, 0, 5], train_parts::COUPLER),
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
        local.push(([0, 1, 2], train_parts::CARGO));
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
                facing,
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
                assert!(parts.iter().all(|part| part.facing == facing));
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
}
