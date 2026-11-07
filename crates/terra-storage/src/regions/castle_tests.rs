use super::*;
use terra_core::castle::{self, connections};

#[test]
fn connected_battlements_preserve_ids_poses_and_neighbor_profiles_through_actual_disk_reload() {
    let root = std::env::temp_dir().join(format!(
        "teramic-castle-connections-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let world = WorldId(76);
    let mut journal = EditJournal::default();
    let mut centers = Vec::new();
    for facing in 0_u8..24 {
        let center = [i64::from(facing) * 48 - 17, 15, -1];
        let material = [
            castle::BERGFRIED_WALL,
            castle::BATTLEMENT_STRAIGHT,
            castle::BATTLEMENT_CORNER,
            castle::BATTLEMENT_PILLAR,
        ][usize::from(facing % 4)];
        journal.record(center, material).unwrap();
        journal
            .set_object_state(
                center,
                Some(terra_world::edits::BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
        for port in [0, 1, 3] {
            let side = connections::port_side(port, facing);
            let mut cell = center;
            cell[side / 2] += if side.is_multiple_of(2) { -1 } else { 1 };
            journal.record(cell, castle::BATTLEMENT_CORNER).unwrap();
            journal
                .set_object_state(
                    cell,
                    Some(terra_world::edits::BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        centers.push((center, material, facing));
    }
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    for (center, material, facing) in centers {
        assert_eq!(loaded.block(center), Some(material));
        assert_eq!(loaded.object_state(center), journal.object_state(center));
        let neighbors = std::array::from_fn(|side| {
            let mut p = center;
            p[side / 2] += if side % 2 == 0 { -1 } else { 1 };
            Some((
                loaded.block(p)?,
                loaded.object_state(p).unwrap_or_default().facing,
            ))
        });
        assert_eq!(
            connections::connection_mask(material, facing, neighbors),
            11
        );
        assert_eq!(
            connections::placed_boxes(material, facing, neighbors),
            connections::profile_boxes(material, 11)
                .iter()
                .map(|b| castle::rotated_box(*b, facing))
                .collect::<Vec<_>>()
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn pillar_stack_bits_are_derived_after_disk_reload_without_new_persisted_state() {
    let root = std::env::temp_dir().join(format!(
        "teramic-pillar-stack-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let world = WorldId(77);
    let mut journal = EditJournal::default();
    let middle = [-1, 15, -1];
    for y in 14..=16 {
        let p = [middle[0], y, middle[2]];
        journal.record(p, castle::BATTLEMENT_PILLAR).unwrap();
        journal
            .set_object_state(p, Some(Default::default()))
            .unwrap();
    }
    let legacy = [0, 15, -1];
    journal.record(legacy, castle::BATTLEMENT_CORNER).unwrap();
    journal
        .set_object_state(legacy, Some(Default::default()))
        .unwrap();
    save_edits(&root, world, &journal).unwrap();
    let mut loaded = load_edits(&root, world).unwrap();
    let derive = |journal: &EditJournal| {
        std::array::from_fn(|side| {
            let mut cell = middle;
            cell[side / 2] += if side % 2 == 0 { -1 } else { 1 };
            Some((
                journal.block(cell)?,
                journal.object_state(cell).unwrap_or_default().facing,
            ))
        })
    };
    assert_eq!(
        connections::profile_key(castle::BATTLEMENT_PILLAR, 0, derive(&loaded)),
        connections::STACK_ABOVE | connections::STACK_BELOW | 2
    );
    assert_eq!(loaded.block(legacy), Some(castle::BATTLEMENT_CORNER));
    loaded.record([middle[0], 16, middle[2]], 0).unwrap();
    assert_eq!(
        connections::profile_key(castle::BATTLEMENT_PILLAR, 0, derive(&loaded)),
        connections::STACK_BELOW | 2
    );
    fs::remove_dir_all(root).unwrap();
}
