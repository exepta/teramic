use super::*;
use crate::{SectionGenerator, TerraGenerator, streaming::LoadedSection};
use terra_voxel::{BlockSection, SectionPos};

fn fixture() -> (WorldTerrain, ChunkStore, EditJournal, [i64; 3]) {
    let mut config = terra_core::WorldConfig {
        base_height: -8.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        biomes: false,
        caves: false,
        trees: false,
        rivers: false,
        landscape_revision: 0,
        ..Default::default()
    };
    config.structures.set_all(false);
    loaded_water(config, [-1, -1])
}

fn loaded_water(
    config: terra_core::WorldConfig,
    [x, z]: [i64; 2],
) -> (WorldTerrain, ChunkStore, EditJournal, [i64; 3]) {
    let terrain = TerraGenerator::new(config).unwrap();
    let tile = [x.div_euclid(16), z.div_euclid(16)];
    let column = SectionGenerator::new(terrain.clone())
        .prepare_column(tile, false)
        .unwrap();
    let surface = column.surface_at(x, z).unwrap();
    let p = [x, surface.water_level.unwrap().ceil() as i64 - 1, z];
    let mut chunks = ChunkStore::default();
    for y in p[1].div_euclid(16)..=(p[1] + 17).div_euclid(16) {
        let pos = SectionPos::new(tile[0], y, tile[1]);
        chunks.loaded.insert(
            pos,
            LoadedSection {
                data: BlockSection::empty(pos).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water: None,
                water_epoch: 0,
                props: vec![],
            },
        );
    }
    chunks.set_prepared_column_for_test(tile, Some(std::sync::Arc::new(column)));
    (WorldTerrain(terrain), chunks, EditJournal::default(), p)
}

#[test]
fn frozen_source_survives_save_replication_thaw_and_manual_breaking() {
    let (terrain, mut chunks, mut journal, p) = fixture();
    let properties = BlockPropertiesCatalog::default();
    let source = chunks.fluid_cell_at(&terrain.0, &journal, p, 0);
    let freezing = candidate(&chunks, &journal, p[0], p[2], -5., 8.0).unwrap();
    assert!(apply(&mut chunks, &mut journal, &properties, freezing).unwrap());
    assert_eq!(chunks.block_at(p), Some(49));
    assert_eq!(chunks.fluid_cell_at(&terrain.0, &journal, p, 49).amount, 0);
    assert_eq!(
        journal.object_state(p).unwrap().frozen_water(),
        Some(source)
    );
    let owner = global_to_section(p).0;
    let saved = serde_json::to_string(&journal.section_delta(owner)).unwrap();
    let (_, mut restored, mut restored_journal, _) = fixture();
    restored
        .apply_confirmed_delta(&mut restored_journal, serde_json::from_str(&saved).unwrap())
        .unwrap();
    assert_eq!(restored.block_at(p), Some(49));
    let thaw = candidate(&restored, &restored_journal, p[0], p[2], 4., 8.0).unwrap();
    assert!(apply(&mut restored, &mut restored_journal, &properties, thaw).unwrap());
    assert_eq!(restored.block_at(p), Some(0));
    assert_eq!(restored_journal.fluid(p), Some(source));
    assert!(restored_journal.object_state(p).is_none());
    chunks
        .apply_confirmed_delta(&mut journal, restored_journal.section_delta(owner))
        .unwrap();
    assert_eq!(chunks.block_at(p), Some(0));
    assert_eq!(chunks.fluid_cell_at(&terrain.0, &journal, p, 0), source);
    let freezing = candidate(&chunks, &journal, p[0], p[2], -5., 8.0).unwrap();
    apply(&mut chunks, &mut journal, &properties, freezing).unwrap();
    commit_block_edit(
        &mut chunks,
        &mut journal,
        &BlockEditRequest {
            position: p,
            expected_material: 49,
            material: 0,
        },
        &properties,
    )
    .unwrap();
    assert_eq!(journal.fluid(p), Some(source));
    assert!(journal.object_state(p).is_none());
}

#[test]
fn player_ice_roofs_withdrawn_sources_and_unknown_air_are_protected() {
    let (_, mut chunks, mut journal, p) = fixture();
    let properties = BlockPropertiesCatalog::default();
    commit_block_edit(
        &mut chunks,
        &mut journal,
        &BlockEditRequest {
            position: p,
            expected_material: 0,
            material: 49,
        },
        &properties,
    )
    .unwrap();
    assert!(candidate(&chunks, &journal, p[0], p[2], 20., 8.0).is_none());
    let (_, mut chunks, mut journal, p) = fixture();
    let roof = [p[0], p[1] + 1, p[2]];
    commit_block_edit(
        &mut chunks,
        &mut journal,
        &BlockEditRequest {
            position: roof,
            expected_material: 0,
            material: 3,
        },
        &properties,
    )
    .unwrap();
    assert!(candidate(&chunks, &journal, p[0], p[2], -10., 8.0).is_none());
    let (_, chunks, mut journal, p) = fixture();
    journal.record(p, 0).unwrap();
    journal.set_fluid(p, Default::default()).unwrap();
    assert!(candidate(&chunks, &journal, p[0], p[2], -10., 8.0).is_none());
    journal
        .set_fluid(
            p,
            crate::water::FluidCell {
                amount: 8,
                source: false,
                flow: [1, 0],
            },
        )
        .unwrap();
    assert!(candidate(&chunks, &journal, p[0], p[2], -10., 8.0).is_none());
    let (_, mut chunks, journal, p) = fixture();
    chunks
        .loaded
        .remove(&SectionPos::new(-1, (p[1] + 16).div_euclid(16), -1));
    assert!(candidate(&chunks, &journal, p[0], p[2], -10., 8.0).is_none());
}

#[test]
fn live_winter_updates_are_budgeted_authoritative_and_pause_with_world_time() {
    let (terrain, chunks, journal, _) = fixture();
    let mut app = App::new();
    app.init_resource::<Time>()
        .insert_resource(WorldAccess::Server)
        .insert_resource(terrain)
        .insert_resource(chunks)
        .insert_resource(journal)
        .insert_resource(WorldClock {
            day: 21,
            ..Default::default()
        })
        .init_resource::<WorldWeather>()
        .init_resource::<BlockPropertiesCatalog>()
        .init_resource::<IceStatus>()
        .add_systems(Update, tick);
    for _ in 0..65 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        app.update();
        let status = app.world().resource::<IceStatus>();
        assert!(status.edits_this_tick <= EDIT_BUDGET);
        assert!(status.scanned_cells <= 256 && status.tracked_cells <= MAX_PROGRESS);
    }
    let freezes = app.world().resource::<IceStatus>().freezes;
    assert!(freezes > 0);
    *app.world_mut().resource_mut::<WorldAccess>() = WorldAccess::Replica { ready: true };
    for _ in 0..80 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        app.update();
    }
    assert_eq!(app.world().resource::<IceStatus>().freezes, freezes);
    *app.world_mut().resource_mut::<WorldAccess>() = WorldAccess::Server;
    app.world_mut().resource_mut::<WorldClock>().paused = true;
    for _ in 0..80 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        app.update();
    }
    assert_eq!(app.world().resource::<IceStatus>().freezes, freezes);
}

#[test]
fn winter_ocean_stays_open_and_only_thaws_marked_weather_ice() {
    let config = terra_core::WorldConfig {
        seed: terra_core::WorldSeed(7863640744109448198),
        rivers: true,
        ..Default::default()
    };
    let (terrain, mut chunks, mut journal, p) = loaded_water(config, [-4579, 5979]);
    let sea = terrain.0.config().sea_level;
    assert_eq!(
        chunks
            .surface_column_at(p[0], p[2])
            .unwrap()
            .biome_kind(sea),
        crate::generation::biomes::BiomeKind::Ocean
    );
    assert!(candidate(&chunks, &journal, p[0], p[2], -25.0, sea).is_none());
    // Recreate a seasonal edit from an earlier build and observe the source returning.
    let source = chunks.fluid_cell_at(&terrain.0, &journal, p, 0);
    let properties = BlockPropertiesCatalog::default();
    assert!(
        apply(
            &mut chunks,
            &mut journal,
            &properties,
            Change {
                position: p,
                material: 0,
                water: source.amount
            }
        )
        .unwrap()
    );
    let thaw = candidate(&chunks, &journal, p[0], p[2], -25.0, sea).unwrap();
    assert!(apply(&mut chunks, &mut journal, &properties, thaw).unwrap());
    assert_eq!(chunks.fluid_cell_at(&terrain.0, &journal, p, 0), source);
    assert!(
        commit_block_edit(
            &mut chunks,
            &mut journal,
            &BlockEditRequest {
                position: p,
                expected_material: 0,
                material: MaterialId::Ice as u16
            },
            &properties
        )
        .unwrap()
    );
    assert!(candidate(&chunks, &journal, p[0], p[2], 10.0, sea).is_none());
}
