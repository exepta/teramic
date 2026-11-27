use super::*;
use crate::streaming::LoadedSection;
use terra_voxel::{BlockSection, SectionPos};

fn chamber() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(WorldAccess::Local)
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .init_resource::<BlockPropertiesCatalog>()
        .add_message::<SectionPublished>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(0.25),
        ))
        .add_systems(Update, tick);
    let pos = SectionPos::default();
    let mut data = BlockSection::empty(pos).unwrap();
    for z in 0..16 {
        for x in 0..16 {
            data.set_block([x, 0, z], M::Stone as u16).unwrap();
        }
    }
    data.set_block([7, 1, 7], LAVA).unwrap();
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        pos,
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: vec![],
        },
    );
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .wake_lava([7, 1, 7]);
    app
}

#[test]
fn placed_pool_only_flows_toward_an_open_bank() {
    let mut app = chamber();
    let mut journal = EditJournal::default();
    journal.record([7, 1, 7], LAVA).unwrap();
    let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
    for z in 6..=7 {
        let data = &mut chunks.loaded.get_mut(&SectionPos::default()).unwrap().data;
        data.set_block([6, 1, z], M::Stone as u16).unwrap();
        data.set_block([7, 1, z], LAVA).unwrap();
    }
    let sealed = surface_corner(&chunks, &journal, [7, 2, 7]);
    assert_eq!(sealed.downhill, [0.; 2]);
    assert_eq!(sealed.height, 1.);
    for z in 6..=7 {
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([6, 1, z], M::Air as u16)
            .unwrap();
    }
    let open = surface_corner(&chunks, &journal, [7, 2, 7]);
    assert!(open.downhill[0] < 0. && open.height < 1.);
    assert_eq!(open.downhill[1], 0.);
}

#[test]
fn reservoir_advances_slowly_recedes_when_cut_and_quenches_on_water() {
    let mut app = chamber();
    app.update();
    app.update();
    assert!(!app.world().resource::<ChunkStore>().lava_at([8, 1, 7]));
    for _ in 0..55 {
        app.update();
    }
    {
        let chunks = app.world().resource::<ChunkStore>();
        let journal = app.world().resource::<EditJournal>();
        for (x, volume) in [(7, 8), (8, 7), (9, 7), (10, 6), (14, 4)] {
            let p = [x, 1, 7];
            let cell = state(chunks, journal, p).unwrap();
            assert_eq!(cell.volume, volume);
            assert!(cell.valid(p, LAVA));
        }
        assert!(chunks.lava_at([15, 1, 7]));
        assert_eq!(
            thermal_damage(chunks, journal, [8.5, 1.2, 7.5], [0.5, 1.8, 0.5], 0.),
            8.
        );
        assert_eq!(
            thermal_damage(chunks, journal, [8.5, 1.99, 7.5], [0.5, 1.8, 0.5], 0.),
            0.
        );
    }
    app.world_mut()
        .resource_scope(|world, mut chunks: Mut<ChunkStore>| {
            let mut journal = world.resource_mut::<EditJournal>();
            // Water abuts the reservoir on top. Only the rock left there persists.
            journal
                .set_fluid(
                    [7, 2, 7],
                    crate::water::FluidCell {
                        amount: 8,
                        source: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            chunks.wake_lava([7, 2, 7]);
        });
    for _ in 0..55 {
        app.update();
    }
    let chunks = app.world().resource::<ChunkStore>();
    assert_eq!(chunks.block_at([7, 1, 7]), Some(M::LavaCrust as u16));
    assert!(!chunks.lava_at([8, 1, 7]) && !chunks.lava_at([10, 1, 7]));
}
#[test]
fn steam_is_local_and_stops_under_a_solid_roof() {
    let mut app = chamber();
    let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
    let base = [3, 1, 3];
    chunks
        .loaded
        .get_mut(&SectionPos::default())
        .unwrap()
        .data
        .set_block([3, 1, 3], FUMAROLE)
        .unwrap();
    let j = EditJournal::default();
    assert!(plume_height(base, 0.) >= 0.48);
    assert_eq!(
        thermal_damage(&chunks, &j, [3.5, 1.9, 3.5], [0.35, 1.8, 0.35], 0.),
        3.
    );
    assert_eq!(
        thermal_damage(&chunks, &j, [4.4, 2., 3.5], [0.35, 1.8, 0.35], 0.),
        0.
    );
    chunks
        .loaded
        .get_mut(&SectionPos::default())
        .unwrap()
        .data
        .set_block([3, 2, 3], M::Stone as u16)
        .unwrap();
    assert_eq!(
        thermal_damage(&chunks, &j, [3.5, 3., 3.5], [0.35, 1.8, 0.35], 0.),
        0.
    );
}

#[test]
fn waterfall_spreads_at_its_foot_and_keeps_the_channel_supplied_across_sections() {
    let mut app = chamber();
    let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
    let data = &mut chunks.loaded.get_mut(&SectionPos::default()).unwrap().data;
    data.set_block([7, 1, 7], 0).unwrap();
    // A high, single-cell channel ends at x=5. The voxel below its lip
    // is airborne, so horizontal outflow at y=4 or y=3 must stay empty.
    for x in 0..5 {
        for y in 1..=4 {
            data.set_block([x, y, 7], M::Stone as u16).unwrap();
        }
        for z in [6, 8] {
            data.set_block([x, 5, z], M::Stone as u16).unwrap();
        }
    }
    data.set_block([1, 5, 7], LAVA).unwrap();
    let east = SectionPos::new(1, 0, 0);
    let mut bed = BlockSection::empty(east).unwrap();
    for x in 0..16 {
        for z in 0..16 {
            bed.set_block([x, 0, z], M::Stone as u16).unwrap();
        }
    }
    chunks.loaded.insert(
        east,
        LoadedSection {
            data: bed,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: vec![],
        },
    );
    chunks.wake_lava([1, 5, 7]);
    for _ in 0..135 {
        app.update();
    }
    let chunks = app.world().resource::<ChunkStore>();
    let journal = app.world().resource::<EditJournal>();
    assert!(chunks.lava_at([5, 5, 7]));
    for y in 2..=4 {
        assert!(chunks.lava_at([5, y, 7]));
        assert!(!chunks.lava_at([6, y, 7]));
    }
    let tip = state(chunks, journal, [19, 1, 7]).expect("14 cells after landing");
    assert_eq!(tip.origin, [1, 5, 7]);
    assert_eq!((tip.age, tip.volume), (14, 1));
    assert!(tip.valid([19, 1, 7], LAVA));
    assert!(!chunks.lava_at([20, 1, 7]));
}

#[test]
fn erupted_melt_spreads_from_a_timed_source_and_cools_while_stone_remains() {
    let mut app = chamber();
    {
        let world = app.world_mut();
        world.resource_scope(|world, mut chunks: Mut<ChunkStore>| {
            let mut journal = world.resource_mut::<EditJournal>();
            for (bed, molten) in [([1, 0, 2], true), ([13, 0, 12], false)] {
                deposit_ejecta(
                    &mut chunks,
                    &mut journal,
                    &BlockPropertiesCatalog::default(),
                    bed,
                    molten,
                );
            }
            assert_eq!(chunks.block_at([1, 1, 2]), Some(LAVA));
            assert_eq!(
                chunks.block_at([13, 1, 12]),
                Some(M::DarkVolcanicStone as u16)
            );
        });
    }
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at([2, 1, 2]),
        Some(LAVA)
    );
    for _ in 0..55 {
        app.update();
    }
    let chunks = app.world().resource::<ChunkStore>();
    assert_eq!(chunks.block_at([1, 1, 2]), Some(M::LavaCrust as u16));
    assert_eq!(
        chunks.block_at([13, 1, 12]),
        Some(M::DarkVolcanicStone as u16)
    );
}
