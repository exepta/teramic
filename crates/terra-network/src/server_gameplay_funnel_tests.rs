use super::*;
use terra_core::{ItemKind, ItemStack, beds::DyeColor, logistics};
use terra_world::{edits::BlockObjectState, streaming::LoadedSection};

#[test]
fn falling_loose_items_land_in_real_funnel_mouths_without_crossing_a_solid_ceiling() {
    let properties = terra_core::block_properties::BlockPropertiesCatalog::load(
        &terra_core::asset_directory().join("terra/blocks"),
    )
    .unwrap();
    for material in [logistics::FUNNEL, logistics::FILTERED_FUNNEL] {
        for obstructed in [false, true] {
            let port = [8, 2, 8];
            let owner = terra_voxel::global_to_section(port).0;
            let mut chunks = ChunkStore::default();
            let mut data = terra_voxel::BlockSection::empty(owner).unwrap();
            data.set_block([8, 2, 8], material).unwrap();
            if obstructed {
                data.set_block([8, 3, 8], 3).unwrap();
            }
            chunks.loaded.insert(
                owner,
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            let mut journal = EditJournal::default();
            journal.record(port, material).unwrap();
            journal
                .set_object_state(port, Some(BlockObjectState::default()))
                .unwrap();
            let stack = ItemStack {
                item: ItemKind::Dye(DyeColor::Rgb([71, 93, 184])),
                count: 11,
            };
            assert!(journal.gameplay_mut().drop_stack(
                [8.5, 5.9, 8.5],
                stack,
                [0.0, -12.0, 0.0],
                0.0,
            ));
            let mut drop = journal.gameplay().drops[0].clone();
            let id = drop.id;
            drop.angular_velocity = [0.0; 3];
            drop.rotation = [0.0, 0.0, 0.0, 1.0];
            for _ in 0..100 {
                step_drop_with_context(&mut drop, &chunks, &journal, &properties, 0.05);
            }
            if obstructed {
                assert!(drop.position[1] - 0.12 >= 4.0 - 0.000001);
                assert_eq!(terra_world::funnels::carrying(&drop, &journal), None);
            } else {
                assert_eq!(drop.position, [8.5, 2.63, 8.5]);
                assert_eq!(drop.velocity, [0.0; 3]);
                assert_eq!(terra_world::funnels::carrying(&drop, &journal), Some(port));
                assert!(drop_bounds_clear_with_context(
                    &chunks,
                    &journal,
                    &properties,
                    drop.position,
                    [0.12; 3],
                ));
            }
            assert_eq!(drop.id, id);
            assert_eq!(drop.stack, stack);
            assert!(drop.valid());
        }
    }
}

#[test]
fn actual_funnel_mouth_support_pauses_lifetime_across_unload_until_the_port_is_removed() {
    use bevy::time::TimeUpdateStrategy;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 60.0,
        )))
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .add_systems(FixedUpdate, simulate_drops);
    let port = [-16, -1, -16];
    let age = terra_core::DROP_LIFETIME_SECONDS - 0.01;
    let stack = ItemStack {
        item: ItemKind::Dye(DyeColor::Rgb([71, 93, 184])),
        count: 11,
    };
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal.record(port, logistics::FILTERED_FUNNEL).unwrap();
        journal
            .set_object_state(port, Some(BlockObjectState::default()))
            .unwrap();
        assert!(
            journal
                .gameplay_mut()
                .drop_stack([-15.5, -0.37, -15.5], stack, [0.0; 3], 0.0)
        );
        let drop = &mut journal.gameplay_mut().drops[0];
        drop.age_seconds = age;
        terra_world::funnels::stabilize(drop, port);
    }
    for _ in 0..30 {
        app.update();
    }
    {
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(journal.gameplay().drops.len(), 1);
        let drop = &journal.gameplay().drops[0];
        assert_eq!(drop.age_seconds, age);
        assert_eq!(drop.stack, stack);
        assert_eq!(terra_world::funnels::carrying(drop, journal), Some(port));
        assert!(journal.gameplay().valid());
    }
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record(port, 0)
        .unwrap();
    for _ in 0..10 {
        app.update();
    }
    assert!(
        app.world()
            .resource::<EditJournal>()
            .gameplay()
            .drops
            .is_empty()
    );
}
