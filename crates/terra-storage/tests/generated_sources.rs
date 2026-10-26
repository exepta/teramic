use terra_core::{PlayerUuid, WorldId};
use terra_world::{edits::EditJournal, generation::structures::StructureObject};

#[test]
fn generated_farm_source_state_and_player_channel_removal_survive_actual_region_files() {
    let path = std::env::temp_dir().join(format!("terra-farm-water-{}", PlayerUuid::new_v4()));
    let world = WorldId(124);
    let objects: Vec<_> = (-4..=2)
        .map(|z| StructureObject {
            position: [-1, 15, z],
            material: 27,
            state: None,
            crop: None,
            loot: None,
        })
        .collect();
    let mut journal = EditJournal::default();
    journal.initialize_structures(&objects).unwrap();
    terra_storage::save_edits(&path, world, &journal).unwrap();
    let mut loaded = terra_storage::load_edits(&path, world).unwrap();
    for object in &objects {
        assert!(loaded.fluid(object.position).unwrap().source);
    }
    assert!(loaded.initialize_structures(&objects).unwrap().is_empty());
    loaded.record(objects[3].position, 0).unwrap();
    terra_storage::save_edits(&path, world, &loaded).unwrap();
    let mut removed = terra_storage::load_edits(&path, world).unwrap();
    assert!(removed.initialize_structures(&objects).unwrap().is_empty());
    assert_eq!(removed.block(objects[3].position), Some(0));
    assert_eq!(removed.fluid(objects[3].position), Some(Default::default()));
    assert_eq!(
        removed
            .fluid_cells()
            .filter(|(_, cell)| cell.source)
            .count(),
        6
    );
    std::fs::remove_dir_all(path).unwrap();
}
