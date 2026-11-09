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

/// The workshop suggests a locomotive shell without making decoration mandatory.
/// Existing cab, engine and frame coordinates stay identical to the functional guide.
pub fn building_template(
    chassis: [i64; 3],
    facing: u8,
    kind: VehicleKind,
    wheelbase: u8,
) -> Vec<BlueprintCell> {
    let mut cells = template_with_wheelbase(chassis, facing, kind, wheelbase);
    if cells.is_empty() || kind != VehicleKind::Locomotive {
        return cells;
    }
    let length = i64::from(wheelbase);
    let mut body = Vec::new();
    for z in 3..length {
        body.push(([0, 1, z], 219, 0));
    }
    body.extend([
        ([0, 1, length], 223, 2),
        ([0, 2, length], 224, 0),
        ([0, 2, if length == 3 { 2 } else { 3 }], 225, 0),
    ]);
    // Walls sit against the outer sides of the existing cab. Two open center
    // columns keep its driver, head space and rear entry free of suggested blocks.
    for z in 0..=1 {
        for x in [-1, 1] {
            let side = if x < 0 { 3 } else { 1 };
            body.push(([x, 0, z], train_parts::FRAME, 0));
            for y in 1..=3 {
                body.push(([x, y, z], if y == 3 { 246 } else { 245 }, side));
            }
        }
        for x in -1..=1 {
            body.push(([x, 4, z], 250, 0));
        }
    }
    cells.extend(body.into_iter().map(|([x, y, z], material, relative)| {
        let [x, z] = match facing {
            0 => [x, z],
            1 => [z, -x],
            2 => [-x, -z],
            _ => [-z, x],
        };
        BlueprintCell {
            cell: [chassis[0] + x, chassis[1] + y, chassis[2] + z],
            material,
            facing: (facing + relative) % 4,
        }
    }));
    cells
}

/// Width is the authored body cross-section; wheelsets retain the shared gauge.
pub fn building_template_with_width(
    chassis: [i64; 3],
    facing: u8,
    kind: VehicleKind,
    wheelbase: u8,
    width: u8,
) -> Vec<BlueprintCell> {
    if !matches!(width, 1 | 3) {
        return Vec::new();
    }
    let mut cells = building_template(chassis, facing, kind, wheelbase);
    if cells.is_empty() {
        return cells;
    }
    let side = if facing.is_multiple_of(2) { 0 } else { 2 };
    if width == 1 {
        cells.retain(|part| part.cell[side] == chassis[side]);
        // Keep a compact canopy above the driver without a three-wide shell.
        for part in &mut cells {
            if part.cell[1] == chassis[1] + 4 {
                part.cell[1] -= 2;
            }
        }
    } else {
        let base = template_with_wheelbase(chassis, facing, kind, wheelbase);
        for part in base
            .iter()
            .filter(|part| matches!(part.material, train_parts::CHASSIS | train_parts::FRAME))
        {
            for offset in [-1, 1] {
                let mut wing = *part;
                wing.cell[side] += offset;
                wing.material = train_parts::FRAME;
                if !cells.iter().any(|old| old.cell == wing.cell) {
                    cells.push(wing);
                }
            }
        }
        for part in &mut cells {
            if crate::train_boiler::is_boiler(part.material) {
                part.facing |= 4;
            }
            if matches!(part.material, 224 | 225) {
                part.cell[1] += 2;
            }
        }
    }
    if kind == VehicleKind::Locomotive && width == 3 {
        // The boiler top sits three cells above its anchor. Put the driver's
        // eye above it, with a continuous floor and outward-facing thin walls.
        cells.retain(|part| !(245..=250).contains(&part.material));
        for part in &mut cells {
            if part.material == train_parts::CAB {
                part.cell[1] += 3;
            }
            if part.material == train_parts::FRAME {
                part.material = 242;
            }
        }
        let mut add = |local: [i64; 3], material: u16, relative: u8| {
            let [x, y, z] = local;
            let [x, z] = match facing {
                0 => [x, z],
                1 => [z, -x],
                2 => [-x, -z],
                _ => [-z, x],
            };
            let cell = [chassis[0] + x, chassis[1] + y, chassis[2] + z];
            if !cells.iter().any(|part| part.cell == cell) {
                cells.push(BlueprintCell {
                    cell,
                    material,
                    facing: (facing + relative) % 4,
                });
            }
        };
        for z in 0..=1 {
            for x in -1..=1 {
                add([x, 3, z], 270, 0);
                add([x, 7, z], 250, 0);
            }
            for x in [-1, 1] {
                for y in 1..=6 {
                    add(
                        [x, y, z],
                        if y >= 5 { 246 } else { 245 },
                        if x < 0 { 1 } else { 3 },
                    );
                }
            }
            for y in 1..=3 {
                add([0, y, z], 270, 0);
            }
        }
        // Front panes are above the boiler, rather than a solid wall at eye level.
        for x in -1..=1 {
            add([x, 5, 2], 246, 2);
            add([x, 6, 2], 246, 2);
            add([x, 7, 2], 250, 0);
        }
        // A rear step reaches the raised floor without blocking the side aisles.
        for z in -3..=-1 {
            for y in 1..=4 + z {
                add([0, y, z], if y == 4 + z { 272 } else { 270 }, 2);
            }
        }
        add([0, 5, 2], 230, 2);
        for y in 2..=3 {
            add([0, y, 2], 270, 0);
        }
        add([0, 4, 2], 229, 0);
        add([0, 5, 0], 231, 2);
        add([-1, 1, -1], 254, 0);
        add([1, 1, -1], 255, 0);
        for x in [-1, 1] {
            add([x, 0, i64::from(wheelbase) + 1], 243, 0);
            add([x, 1, i64::from(wheelbase) + 1], 264, 2);
        }
    }
    cells
}

/// The workshop requests an actual model preview without creating a world vehicle.
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Default, PartialEq)]
pub struct WorkshopPreviewRequest {
    pub visible: bool,
    pub parts: Vec<crate::trains::TrainPart>,
    pub blocks: std::collections::BTreeMap<u16, crate::trains::TrainBlockView>,
}

/// Green ghosts disappear only when the correct part and required axis are present.
pub fn matches(cell: BlueprintCell, material: Option<u16>, facing: u8) -> bool {
    material == Some(cell.material)
        && match cell.material {
            train_parts::BOGIE | train_parts::COUPLER => facing % 2 == cell.facing % 2,
            train_parts::CAB | train_parts::SEAT => facing == cell.facing,
            train_parts::ENGINE => true,
            material if crate::train_boiler::is_boiler(material) => {
                facing < 8 && facing == cell.facing
            }
            material if crate::locomotive_parts::is_part(material) => facing == cell.facing,
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
                        .filter(|part| matches!(part.material, train_parts::FRAME | 242))
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
                    assert!(parts.iter().any(|part| part.cell == above
                        && matches!(part.material, train_parts::FRAME | 242)));
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

    #[test]
    fn optional_locomotive_shell_preserves_the_skeleton_and_driver_headroom_in_every_pose() {
        for wheelbase in MIN_WHEELBASE..=MAX_WHEELBASE {
            for facing in 0..4 {
                let skeleton =
                    template_with_wheelbase([0; 3], facing, VehicleKind::Locomotive, wheelbase);
                let guide = building_template([0; 3], facing, VehicleKind::Locomotive, wheelbase);
                assert!(guide.starts_with(&skeleton));
                assert_eq!(guide.len(), usize::from(wheelbase) * 2 + 31);
                assert!(guide.len() < crate::trains::MAX_PARTS);
                let cells: BTreeSet<_> = guide.iter().map(|part| part.cell).collect();
                assert_eq!(cells.len(), guide.len());
                let turn = |[x, y, z]: [i64; 3]| match facing {
                    0 => [x, y, z],
                    1 => [z, y, -x],
                    2 => [-x, y, -z],
                    _ => [-z, y, x],
                };
                for z in 0..=1 {
                    for y in 2..=3 {
                        assert!(
                            !cells.contains(&turn([0, y, z])),
                            "cab head and rear entry stay clear"
                        );
                    }
                    for x in -1..=1 {
                        assert!(
                            guide
                                .iter()
                                .any(|part| part.cell == turn([x, 4, z]) && part.material == 250)
                        );
                    }
                }
                assert!(
                    guide
                        .iter()
                        .any(|part| part.material == 223 && part.facing == (facing + 2) % 4)
                );
                assert!(guide.iter().any(|part| part.material == 224));
                let mut connected = BTreeSet::new();
                let mut pending = vec![guide[0].cell];
                while let Some(cell) = pending.pop() {
                    if !connected.insert(cell) {
                        continue;
                    }
                    for axis in 0..3 {
                        for step in [-1, 1] {
                            let mut neighbor = cell;
                            neighbor[axis] += step;
                            if cells.contains(&neighbor) && !connected.contains(&neighbor) {
                                pending.push(neighbor);
                            }
                        }
                    }
                }
                assert_eq!(
                    connected, cells,
                    "the optional shell has no disconnected cells"
                );
            }
        }
        assert!(building_template([0; 3], 4, VehicleKind::Locomotive, 4).is_empty());
        assert_eq!(
            building_template([0; 3], 0, VehicleKind::Freight, 4),
            template([0; 3], 0, VehicleKind::Freight)
        );
    }

    #[test]
    fn matching_directional_body_parts_requires_the_guides_actual_orientation() {
        let wall = BlueprintCell {
            cell: [0; 3],
            material: 245,
            facing: 3,
        };
        assert!(matches(wall, Some(245), 3));
        for facing in 0..3 {
            assert!(!matches(wall, Some(245), facing));
        }
        assert!(!matches(wall, Some(246), 3));
        for facing in 0..4 {
            for material in [train_parts::CHASSIS, train_parts::ENGINE] {
                assert!(matches(
                    BlueprintCell {
                        material,
                        facing: 0,
                        ..wall
                    },
                    Some(material),
                    facing
                ));
            }
        }
    }

    #[test]
    fn optional_cab_shell_keeps_native_driver_headroom_and_side_window_eye_rays_clear() {
        use crate::block_shapes::ShapeBox;
        for wheelbase in [3, 4, 15] {
            for facing in 0..4 {
                let skeleton =
                    template_with_wheelbase([0; 3], facing, VehicleKind::Locomotive, wheelbase);
                let guide = building_template([0; 3], facing, VehicleKind::Locomotive, wheelbase);
                let head = ShapeBox {
                    minimum: [0.16, 1.485, 1.15],
                    maximum: [0.84, 3.285, 1.85],
                    hinged: false,
                }
                .rotated_y(facing);
                // Offset from the window's central decorative mullion. The
                // actual cab eye is 1+.485+.9+.72=3.105 above the chassis cell.
                let rays = [(-0.2, 0.5), (0.5, 1.2)].map(|(left, right)| {
                    ShapeBox {
                        minimum: [left, 3.10, 1.345],
                        maximum: [right, 3.11, 1.355],
                        hinged: false,
                    }
                    .rotated_y(facing)
                });
                for part in guide.iter().skip(skeleton.len()) {
                    for mut bounds in train_parts::boxes(part.material, part.facing) {
                        for axis in 0..3 {
                            bounds.minimum[axis] += part.cell[axis] as f64;
                            bounds.maximum[axis] += part.cell[axis] as f64;
                        }
                        assert!(
                            !bounds.intersects(head.minimum, head.maximum),
                            "optional part {} clips driver head space",
                            part.material
                        );
                        for ray in rays {
                            assert!(
                                !bounds.intersects(ray.minimum, ray.maximum),
                                "optional part {} blocks the driver's actual window eye ray",
                                part.material
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn body_width_is_explicit_in_all_yaws_and_does_not_resize_the_wheelsets() {
        for facing in 0..4 {
            for kind in [
                VehicleKind::Locomotive,
                VehicleKind::Tender,
                VehicleKind::Passenger,
                VehicleKind::Freight,
            ] {
                let narrow = building_template_with_width([0; 3], facing, kind, 4, 1);
                let wide = building_template_with_width([0; 3], facing, kind, 4, 3);
                let side = if facing % 2 == 0 { 0 } else { 2 };
                assert!(narrow.iter().all(|part| part.cell[side] == 0));
                assert!(wide.iter().any(|part| part.cell[side] == -1
                    && matches!(part.material, train_parts::FRAME | 242)));
                assert!(wide.iter().any(|part| part.cell[side] == 1
                    && matches!(part.material, train_parts::FRAME | 242)));
                let bogies = |cells: &[BlueprintCell]| {
                    cells
                        .iter()
                        .filter(|part| part.material == train_parts::BOGIE)
                        .copied()
                        .collect::<Vec<_>>()
                };
                assert_eq!(bogies(&narrow), bogies(&wide));
                assert!(
                    wide.iter()
                        .filter(|part| crate::train_boiler::is_boiler(part.material))
                        .all(|part| part.facing >= 4)
                );
                assert!(
                    narrow
                        .iter()
                        .filter(|part| crate::train_boiler::is_boiler(part.material))
                        .all(|part| part.facing < 4)
                );
            }
        }
        assert!(building_template_with_width([0; 3], 0, VehicleKind::Locomotive, 4, 2).is_empty());
    }
    #[test]
    fn wide_cab_has_clear_forward_windows_above_the_boiler_and_outward_side_walls() {
        for facing in 0..4 {
            let guide = building_template_with_width([0; 3], facing, VehicleKind::Locomotive, 8, 3);
            let cab = guide
                .iter()
                .find(|p| p.material == train_parts::CAB)
                .unwrap();
            let highest_boiler = guide
                .iter()
                .filter(|p| crate::train_boiler::is_boiler(p.material))
                .flat_map(|p| {
                    crate::train_boiler::boxes(p.material, p.facing)
                        .into_iter()
                        .map(move |b| p.cell[1] as f64 + b.maximum[1])
                })
                .fold(0.0, f64::max);
            // Seat feet + half body height + eye offset, as used by the
            // mounted character controller. Leave clearance for the whole body.
            assert!(cab.cell[1] as f64 + 0.485 + 0.9 + 0.72 > highest_boiler);
            let head = crate::block_shapes::ShapeBox {
                minimum: [0.16, cab.cell[1] as f64 + 0.485, 1.15],
                maximum: [0.84, cab.cell[1] as f64 + 0.485 + 1.8, 1.85],
                hinged: false,
            }
            .rotated_y(facing);
            for part in guide.iter().filter(|p| p.material != train_parts::CAB) {
                for mut bounds in train_parts::boxes(part.material, part.facing) {
                    for axis in 0..3 {
                        bounds.minimum[axis] += part.cell[axis] as f64;
                        bounds.maximum[axis] += part.cell[axis] as f64;
                    }
                    assert!(
                        !bounds.intersects(head.minimum, head.maximum),
                        "wide cab part {} clips driver",
                        part.material
                    );
                }
            }
            assert!(guide.iter().any(|p| p.material == 246 && p.cell[1] == 6));
            assert!(guide.iter().any(|p| p.material == 250 && p.cell[1] == 7));
            let parts: Vec<_> = guide
                .iter()
                .map(|p| crate::trains::TrainPart {
                    offset: p.cell.map(|v| v as i16),
                    material: p.material,
                    facing: p.facing,
                    wood: Default::default(),
                })
                .collect();
            assert!(crate::train_power::steam_installed(&parts));
        }
    }
}
