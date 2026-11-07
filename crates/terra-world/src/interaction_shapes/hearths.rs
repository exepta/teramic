use terra_core::block_shapes::ShapeBox;

pub const ATTACHED_FIRE_SCALE: f64 = 0.64;

/// The existing fire fits inside the kettle frame without changing its ground height.
pub fn attached_campfire_point(point: [f64; 3]) -> [f64; 3] {
    [
        0.5 + (point[0] - 0.5) * ATTACHED_FIRE_SCALE,
        point[1],
        0.5 + (point[2] - 0.5) * ATTACHED_FIRE_SCALE,
    ]
}

pub fn attached_campfire_box(bounds: ShapeBox) -> ShapeBox {
    ShapeBox {
        minimum: attached_campfire_point(bounds.minimum),
        maximum: attached_campfire_point(bounds.maximum),
        hinged: bounds.hinged,
    }
}

/// Four stone piers and their ground-contacting pads, beneath the cauldron's feet.
pub fn campfire_support_boxes() -> [ShapeBox; 8] {
    std::array::from_fn(|index| {
        let x = if index / 2 % 2 == 0 { 0.11 } else { 0.89 };
        let z = if index / 4 == 0 { 0.11 } else { 0.89 };
        let (half, bottom, top) = if index % 2 == 0 {
            (0.11, 0.0, 0.20)
        } else {
            (0.075, 0.16, 1.0)
        };
        ShapeBox {
            minimum: [x - half, bottom, z - half],
            maximum: [x + half, top, z + half],
            hinged: false,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use terra_voxel::global_to_section;

    #[test]
    fn attached_fire_picking_and_collision_share_the_same_compact_geometry_in_every_yaw() {
        let properties =
            BlockPropertiesCatalog::load(&terra_core::asset_directory().join("terra/blocks"))
                .unwrap();
        for position in [[0, 17, 0], [-16, -1, -16]] {
            let (section, local) = global_to_section(position);
            let mut chunks = ChunkStore::default();
            let mut data = terra_voxel::BlockSection::empty(section).unwrap();
            data.set_block(local, 161).unwrap();
            chunks.loaded.insert(
                section,
                crate::streaming::LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            let mut journal = EditJournal::default();
            journal.record(position, 161).unwrap();
            for facing in 0..8 {
                journal
                    .set_object_state(
                        position,
                        Some(crate::edits::BlockObjectState {
                            facing,
                            ..Default::default()
                        }),
                    )
                    .unwrap();
                let mut picking = Vec::new();
                let mut collision = Vec::new();
                assert!(visit_boxes(&chunks, &journal, &properties, position, |b| {
                    picking.push(b)
                }));
                assert!(visit_collision_boxes(
                    &chunks,
                    &journal,
                    &properties,
                    position,
                    |b| collision.push(b)
                ));
                assert_eq!(picking, collision);
                let attached = terra_core::brewing::has_fire(facing);
                assert_eq!(
                    picking.len(),
                    properties.shape(161).len()
                        + usize::from(attached) * properties.shape(162).len()
                );
                if !attached {
                    continue;
                }
                for (actual, source) in picking.iter().zip(properties.shape(162)) {
                    let mut expected = attached_campfire_box(*source).rotated_y(facing % 4);
                    assert!(expected.minimum[0] > 0.175 && expected.minimum[2] > 0.175);
                    assert!(expected.maximum[0] < 0.825 && expected.maximum[2] < 0.825);
                    assert!(expected.maximum[1] < 0.34, "the bowl bottom stays clear");
                    for (axis, value) in position.into_iter().enumerate() {
                        expected.minimum[axis] += value as f64;
                        expected.maximum[axis] += value as f64;
                    }
                    assert_eq!(*actual, expected);
                }
            }
        }
    }

    #[test]
    fn cauldron_piers_touch_the_ground_and_feet_without_filling_the_fire() {
        let boxes = campfire_support_boxes();
        for pair in boxes.chunks_exact(2) {
            assert_eq!(pair[0].minimum[1], 0.0);
            assert!(pair[0].maximum[1] >= pair[1].minimum[1]);
            assert_eq!(pair[1].maximum[1], 1.0);
            for bounds in pair {
                assert!(!bounds.intersects([0.4, 0.0, 0.4], [0.6, 1.0, 0.6]));
                assert!(
                    (0..3).all(|axis| bounds.minimum[axis] >= 0.0 && bounds.maximum[axis] <= 1.0)
                );
            }
        }
    }

    #[test]
    fn covered_fire_collision_matches_piers_only_with_a_loaded_kettle_above() {
        let properties = BlockPropertiesCatalog::default();
        for position in [[0, 17, 0], [-16, -1, -16]] {
            let above = [position[0], position[1] + 1, position[2]];
            let mut journal = EditJournal::default();
            journal.record(position, 162).unwrap();
            // Persisted but unloaded metadata must not manufacture physical support.
            journal.record(above, 161).unwrap();
            for upper in [None, Some(0), Some(3), Some(161)] {
                let mut actual = Vec::new();
                assert!(visit_boxes_with(
                    &journal,
                    &properties,
                    position,
                    |cell| {
                        if cell == position {
                            Some(162)
                        } else if cell == above {
                            upper
                        } else {
                            Some(0)
                        }
                    },
                    |bounds| actual.push(bounds)
                ));
                let supports: Vec<_> = actual
                    .iter()
                    .filter(|bounds| bounds.maximum[1] > position[1] as f64 + 0.5)
                    .copied()
                    .collect();
                let expected: Vec<_> = if upper == Some(161) {
                    campfire_support_boxes()
                        .map(|mut bounds| {
                            for (axis, value) in position.into_iter().enumerate() {
                                bounds.minimum[axis] += value as f64;
                                bounds.maximum[axis] += value as f64;
                            }
                            bounds
                        })
                        .into_iter()
                        .filter(|bounds| bounds.maximum[1] > position[1] as f64 + 0.5)
                        .collect()
                } else {
                    Vec::new()
                };
                assert_eq!(supports, expected);
                assert_eq!(
                    actual.len(),
                    properties.shape(162).len() + usize::from(upper == Some(161)) * 8
                );
            }
        }
    }
}
