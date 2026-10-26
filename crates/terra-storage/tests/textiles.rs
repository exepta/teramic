use terra_core::{PlayerUuid, WorldId, beds::DyeColor};
use terra_world::edits::{BlockObjectState, EditJournal};

#[test]
fn colored_textiles_and_every_pot_content_survive_real_region_reload_and_air_tombstones() {
    let path = std::env::temp_dir().join(format!("terra-textiles-{}", PlayerUuid::new_v4()));
    let world = WorldId(124);
    let mut journal = EditJournal::default();
    let mut positions = Vec::new();
    for (index, color) in DyeColor::ALL.into_iter().enumerate() {
        for base in [
            terra_core::textiles::WHITE_WOOL,
            terra_core::textiles::WHITE_CARPET,
        ] {
            let position = [-33 + index as i64, 15, if base == 103 { -17 } else { -18 }];
            let material = terra_core::textiles::dyed(base, color).unwrap();
            journal.record(position, material).unwrap();
            positions.push((position, material));
        }
    }
    for facing in 0..terra_core::flower_pot::STATES {
        let position = [-48 + i64::from(facing), -1, -32];
        journal
            .record(position, terra_core::flower_pot::MATERIAL)
            .unwrap();
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    terra_storage::save_edits(&path, world, &journal).unwrap();
    let mut loaded = terra_storage::load_edits(&path, world).unwrap();
    for (position, material) in positions {
        assert_eq!(loaded.block(position), Some(material));
    }
    for facing in 0..terra_core::flower_pot::STATES {
        let position = [-48 + i64::from(facing), -1, -32];
        assert_eq!(loaded.object_state(position).unwrap().facing, facing);
        loaded.record(position, 0).unwrap();
    }
    terra_storage::save_edits(&path, world, &loaded).unwrap();
    let removed = terra_storage::load_edits(&path, world).unwrap();
    for facing in 0..terra_core::flower_pot::STATES {
        let position = [-48 + i64::from(facing), -1, -32];
        assert_eq!(removed.block(position), Some(0));
        assert_eq!(removed.object_state(position), None);
    }
    std::fs::remove_dir_all(path).unwrap();
}
