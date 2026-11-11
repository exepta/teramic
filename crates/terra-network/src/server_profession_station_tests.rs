use super::*;

fn current(app: &mut App, position: [i64; 3]) -> Option<ContainerSnapshot> {
    let mut system = bevy::ecs::system::SystemState::<ControlWorld>::new(app.world_mut());
    snapshot(&system.get_mut(app.world_mut()).unwrap(), position)
}

#[test]
fn every_profession_station_opens_its_own_real_single_cell_depot() {
    let config = GameConfig::default();
    let mut app = App::new();
    app.insert_resource(terra_world::streaming::WorldTerrain(
        terra_world::TerraGenerator::new(config.world.clone()).unwrap(),
    ))
    .insert_resource(terra_core::WorldClock::new(config.world.time))
    .init_resource::<terra_core::WorldWeather>()
    .insert_resource(config)
    .init_resource::<terra_core::crafting::RecipeBook>()
    .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
    .init_resource::<ConnectedProfiles>()
    .init_resource::<riding::Mounts>();
    let section = terra_voxel::SectionPos::default();
    let mut data = terra_voxel::BlockSection::empty(section).unwrap();
    let mut journal = EditJournal::default();
    for (offset, material) in terra_core::profession_stations::ALL.into_iter().enumerate() {
        let position = [offset as i64, 1, 4];
        data.set_block(terra_voxel::global_to_section(position).1, material)
            .unwrap();
        journal.record(position, material).unwrap();
        journal
            .set_object_state(
                position,
                Some(terra_world::edits::BlockObjectState {
                    facing: (offset % 4) as u8,
                    ..Default::default()
                }),
            )
            .unwrap();
        let mut contents = terra_core::Inventory::default();
        contents.insert(terra_core::ItemKind::Stick, offset as u16 + 1);
        journal.set_container(position, contents).unwrap();
    }
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        section,
        terra_world::streaming::LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    app.insert_resource(chunks).insert_resource(journal);
    for offset in 0..9 {
        let position = [offset as i64, 1, 4];
        let view = current(&mut app, position).unwrap();
        assert!(view.valid());
        assert_eq!(view.anchor, position);
        assert_eq!(view.halves.len(), 1);
        assert!(view.halves[0].inventory.slots.len() <= terra_core::containers::CHEST_SLOTS);
        assert_eq!(
            view.halves[0].inventory.count(terra_core::ItemKind::Stick),
            offset as u16 + 1
        );
        assert!(view.workbench.is_none() && view.furnace.is_none() && view.crusher.is_none());
    }
}
