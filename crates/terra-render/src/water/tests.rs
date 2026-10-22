use super::{material::*, upload::*};
use bevy::prelude::*;
use terra_core::{GameConfig, WaterQuality};
use terra_voxel::{BlockSection, SectionPos};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
    water::WaterPatch,
};

fn app() -> App {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<WaterMaterial>>()
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .init_resource::<GameConfig>()
        .init_resource::<WaterTiles>()
        .init_resource::<WaterStatus>()
        .add_systems(Startup, setup_water)
        .add_systems(Update, update_water_settings)
        .add_systems(Update, upload_water);
    app
}

#[test]
fn water_quality_changes_reuse_the_material_and_preserve_transmission() {
    let mut app = app();
    app.update();
    let handle = app.world().resource::<WaterAssets>().0.clone();
    for (quality, wave_count, caustics, reflection_steps) in [
        (WaterQuality::Low, 1.0, false, 0.0),
        (WaterQuality::Medium, 2.0, false, 16.0),
        (WaterQuality::High, 3.0, true, 28.0),
    ] {
        {
            let mut config = app.world_mut().resource_mut::<GameConfig>();
            config.client.water_quality = quality;
            config.client.water_wave_strength = 0.0;
        }
        app.update();
        let materials = app.world().resource::<Assets<WaterMaterial>>();
        assert_eq!(materials.len(), 1);
        let water = materials.get(&handle).unwrap();
        assert_eq!(water.base.specular_transmission, 1.0);
        assert_eq!(water.extension.effects.x, 0.0);
        assert_eq!(water.extension.effects.y, wave_count);
        assert_eq!(water.extension.effects.z > 0.0, caustics);
        assert_eq!(water.extension.reflections.x, reflection_steps);
        assert!(water.extension.reflections.z > 0.0);
        assert!(water.extension.reflections.w > 0.0);
        assert!(water.extension.settings.is_finite());
        assert!(water.extension.effects.is_finite());
        assert!(water.extension.reflections.is_finite());
        assert!(water.extension.weather.is_finite());
    }
    app.world_mut()
        .resource_mut::<GameConfig>()
        .client
        .water_reflections = false;
    app.update();
    let materials = app.world().resource::<Assets<WaterMaterial>>();
    assert_eq!(materials.len(), 1);
    assert_eq!(materials.get(&handle).unwrap().extension.reflections.x, 0.0);
}

#[test]
fn storms_change_one_shared_water_uniform_without_recreating_materials() {
    let mut app = app();
    app.insert_resource(crate::weather::WeatherVisual {
        wind: Vec2::new(0.35, 0.15),
        ..default()
    });
    app.update();
    let handle = app.world().resource::<WaterAssets>().0.clone();
    let calm = app
        .world()
        .resource::<Assets<WaterMaterial>>()
        .get(&handle)
        .unwrap()
        .extension
        .weather;
    app.world_mut()
        .resource_mut::<crate::weather::WeatherVisual>()
        .wind = Vec2::new(-6.4, 4.8);
    app.update();
    let materials = app.world().resource::<Assets<WaterMaterial>>();
    assert_eq!(materials.len(), 1);
    let storm = materials.get(&handle).unwrap().extension.weather;
    assert_eq!(calm.z, 0.0);
    assert_eq!(storm.z, 1.0);
    assert!(storm.x < 0.0 && storm.y > 0.0);
    assert!(storm.w > calm.w);
    assert!((storm.xy().length() - 1.0).abs() < 0.01);
}
fn section(epoch: u64) -> LoadedSection {
    LoadedSection {
        data: BlockSection::empty(SectionPos::default()).unwrap(),
        mesh: None,
        mesh_epoch: epoch,
        water_epoch: epoch,
        props: Vec::new(),
        water: Some(WaterPatch {
            normals: Vec::new(),
            flow: Vec::new(),
            surface_effects: Vec::new(),
            wave_weights: Vec::new(),
            positions: vec![[0.0, 8.0, 0.0], [0.0, 8.0, 1.0], [1.0, 8.0, 0.0]],
            depth_shore: vec![[4.0, 0.0]; 3],
            indices: vec![0, 1, 2],
        }),
    }
}

#[test]
fn empty_fluid_rebuild_removes_the_old_tile_without_uploading_an_empty_mesh() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(SectionPos::default(), section(0));
    app.update();
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_fluid(
            [8, 8, 8],
            terra_world::water::FluidCell {
                amount: 8,
                source: true,
                flow: [0; 2],
            },
        )
        .unwrap();
    let epoch = app
        .world()
        .resource::<EditJournal>()
        .water_mesh_epoch(SectionPos::default());
    let mut empty = section(epoch);
    let patch = empty.water.as_mut().unwrap();
    patch.positions.clear();
    patch.depth_shore.clear();
    patch.indices.clear();
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(SectionPos::default(), empty);
    let mut exhausted = crate::MeshUploadStatus::default();
    for _ in 0..4 {
        assert!(exhausted.reserve(crate::upload_budget::UploadKind::Water, 1));
    }
    app.insert_resource(exhausted);
    app.update();
    assert!(app.world().resource::<Assets<Mesh>>().is_empty());
    let status = app.world().resource::<WaterStatus>();
    assert!(status.ready && status.initial_ready);
    assert_eq!(status.visible_tiles, 0);
    assert_eq!(status.triangles, 0);
    assert_eq!(
        app.world()
            .resource::<crate::MeshUploadStatus>()
            .deferred_batches,
        0
    );
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<WaterSurface>>()
            .iter(app.world())
            .count(),
        0
    );
    app.update();
    assert!(app.world().resource::<Assets<Mesh>>().is_empty());
}

#[test]
fn deferred_water_uploads_finish_without_another_chunk_or_journal_change() {
    let mut app = app();
    for x in -4..=4 {
        let position = SectionPos::new(x, 0, -1);
        let mut water = section(0);
        water.data = BlockSection::empty(position).unwrap();
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(position, water);
    }
    app.update();
    assert_eq!(app.world().resource::<WaterStatus>().visible_tiles, 4);
    assert!(!app.world().resource::<WaterStatus>().initial_ready);
    app.update();
    assert_eq!(app.world().resource::<WaterStatus>().visible_tiles, 8);
    assert!(!app.world().resource::<WaterStatus>().ready);
    app.update();
    let status = app.world().resource::<WaterStatus>();
    assert_eq!(status.visible_tiles, 9);
    assert!(status.ready && status.initial_ready);
    app.update();
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 9);
}

#[test]
fn repeated_water_rebuilds_and_unloads_release_meshes_but_reuse_the_material() {
    let mut app = app();
    app.update();
    let mut entities = app.world_mut().query::<Entity>();
    let baseline_entities = entities.iter(app.world()).count();
    let material = app.world().resource::<WaterAssets>().0.clone();
    let owner = SectionPos::default();
    for cycle in 0..100 {
        let mut previous = None;
        for amount in [4, 8] {
            app.world_mut()
                .resource_mut::<EditJournal>()
                .set_fluid(
                    [8, 8, 8],
                    terra_world::water::FluidCell {
                        amount,
                        source: false,
                        flow: [1, 0],
                    },
                )
                .unwrap();
            let epoch = app
                .world()
                .resource::<EditJournal>()
                .water_mesh_epoch(owner);
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .insert(owner, section(epoch));
            app.update();
            let world = app.world_mut();
            assert_eq!(world.resource::<Assets<Mesh>>().len(), 1, "cycle {cycle}");
            assert_eq!(entities.iter(world).count(), baseline_entities + 1);
            assert_eq!(world.query::<&WaterSurface>().iter(world).count(), 1);
            assert_eq!(world.resource::<WaterStatus>().visible_tiles, 1);
            if let Some(old) = previous.take() {
                assert!(world.resource::<Assets<Mesh>>().get(old).is_none());
            }
            previous = world
                .resource::<Assets<Mesh>>()
                .iter()
                .next()
                .map(|(id, _)| id);
            assert_eq!(world.resource::<Assets<WaterMaterial>>().len(), 1);
            assert_eq!(world.resource::<WaterAssets>().0, material);
        }
        app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
        app.update();
        let world = app.world_mut();
        assert!(world.resource::<Assets<Mesh>>().is_empty());
        assert_eq!(
            entities.iter(world).count(),
            baseline_entities,
            "cycle {cycle}"
        );
        assert_eq!(world.query::<&WaterSurface>().iter(world).count(), 0);
        assert_eq!(world.resource::<WaterStatus>().visible_tiles, 0);
        assert!(world.resource::<WaterStatus>().ready);
        assert_eq!(world.resource::<Assets<WaterMaterial>>().len(), 1);
    }
}

#[test]
fn initial_water_readiness_waits_for_first_upload_but_not_later_fluid_updates() {
    use terra_world::water::FluidCell;

    let mut app = app();
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(SectionPos::default(), section(0));
    app.update();
    assert!(app.world().resource::<WaterStatus>().initial_ready);

    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_fluid(
            [15, 8, 8],
            FluidCell {
                amount: 4,
                source: false,
                flow: [1, 0],
            },
        )
        .unwrap();
    app.update();
    let status = app.world().resource::<WaterStatus>();
    assert!(!status.ready);
    assert!(status.initial_ready);

    let neighbor = SectionPos::new(1, 0, 0);
    let mut unprepared = section(0);
    unprepared.data = BlockSection::empty(neighbor).unwrap();
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(neighbor, unprepared);
    app.update();
    assert!(!app.world().resource::<WaterStatus>().initial_ready);

    let epoch = app
        .world()
        .resource::<EditJournal>()
        .water_mesh_epoch(neighbor);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&neighbor)
        .unwrap()
        .water_epoch = epoch;
    app.update();
    let status = app.world().resource::<WaterStatus>();
    assert!(!status.ready);
    assert!(status.initial_ready);
    assert_eq!(status.visible_tiles, 2);

    app.world_mut().resource_mut::<GameConfig>().world.water = false;
    app.update();
    let status = app.world().resource::<WaterStatus>();
    assert!(status.ready && status.initial_ready);
}

#[test]
fn biome_upload_keeps_flow_foam_and_the_shared_material() {
    use bevy::mesh::VertexAttributeValues;
    use terra_world::{TerraGenerator, streaming::WorldTerrain};

    let mut app = app();
    app.insert_resource(WorldTerrain(
        TerraGenerator::new(Default::default()).unwrap(),
    ));
    let flow = vec![[0.3, 0.7]; 3];
    let effects = vec![[0.4, 1.2, 0.8, 5.0]; 3];
    let mut tile = section(0);
    let patch = tile.water.as_mut().unwrap();
    patch.flow = flow.clone();
    patch.surface_effects = effects.clone();
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(SectionPos::default(), tile);
    app.update();
    let world = app.world();
    assert_eq!(world.resource::<Assets<WaterMaterial>>().len(), 1);
    let (_, mesh) = world.resource::<Assets<Mesh>>().iter().next().unwrap();
    assert_eq!(
        mesh.attribute(Mesh::ATTRIBUTE_UV_1),
        Some(&VertexAttributeValues::Float32x2(flow))
    );
    assert_eq!(
        mesh.attribute(Mesh::ATTRIBUTE_COLOR),
        Some(&VertexAttributeValues::Float32x4(effects))
    );
    let Some(VertexAttributeValues::Float32x3(colors)) =
        mesh.attribute(super::biome::ATTRIBUTE_BIOME_TINT)
    else {
        panic!("water must carry its own biome tint, not reuse current/foam attributes");
    };
    let palette = super::biome::WaterTint::new(&world.resource::<WorldTerrain>().0, [0; 3]);
    for (color, point) in colors
        .iter()
        .zip([[0.0, 8.0, 0.0], [0.0, 8.0, 1.0], [1.0, 8.0, 0.0]])
    {
        assert_eq!(*color, palette.at(point));
    }
}

#[test]
fn solid_water_blocks_share_the_surface_palette_without_current_uvs() {
    use crate::{
        blocks::{
            BlockAssets, BlockTemplate, TemplateSet, cube_faces, definitions::BlockRenderKind,
        },
        meshing::{
            GeometryCache, RenderJobs, RenderStatus, RenderedSections, dispatch_meshes,
            upload_sections,
        },
    };
    use bevy::{app::TaskPoolPlugin, mesh::VertexAttributeValues};
    use std::{sync::Arc, time::Duration};
    use terra_world::{
        TerraGenerator,
        streaming::{ChunkFocus, WorldTerrain},
    };

    let mut app = app();
    app.add_plugins(TaskPoolPlugin::default())
        .init_resource::<BlockAssets>()
        .init_resource::<RenderedSections>()
        .init_resource::<RenderStatus>()
        .init_resource::<RenderJobs>()
        .init_resource::<GeometryCache>()
        .insert_resource(ChunkFocus([8.0, 8.0, 8.0]))
        .insert_resource(WorldTerrain(
            TerraGenerator::new(Default::default()).unwrap(),
        ))
        .add_systems(Update, (upload_sections, dispatch_meshes).chain());
    let mut templates = TemplateSet::default();
    templates.blocks.insert(
        14,
        BlockTemplate {
            surface: None,
            render_kind: BlockRenderKind::Cube,
            bevel_width: 0.0,
            faces: cube_faces(0.0, [0; 6], None),
            face_materials: [0; 6],
            side_cap: None,
            details: Vec::new(),
            model: Vec::new(),
        },
    );
    app.world_mut().resource_mut::<BlockAssets>().templates = Some(Arc::new(templates));
    let p = SectionPos::default();
    let mut tile = section(0);
    tile.water = None;
    tile.mesh = Some(terra_voxel::meshing::BlockMesh {
        faces: vec![terra_voxel::meshing::BlockFace {
            minimum: [0; 3],
            width: 1,
            height: 1,
            u_axis: 0,
            v_axis: 2,
            side: 3,
            material: 14,
            edge_mask: 15,
            occupied_neighbor: false,
        }],
        ..default()
    });
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(p, tile);
    for _ in 0..500 {
        app.update();
        if !app
            .world()
            .resource::<RenderedSections>()
            .sections
            .is_empty()
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let world = app.world_mut();
    assert!(world.resource::<RenderStatus>().error.is_none());
    let rendered = &world.resource::<RenderedSections>().sections[&p];
    let mesh = world
        .resource::<Assets<Mesh>>()
        .get(&rendered.meshes[0])
        .unwrap();
    assert!(mesh.attribute(Mesh::ATTRIBUTE_UV_1).is_none());
    assert!(matches!(mesh.attribute(super::biome::ATTRIBUTE_BIOME_TINT),
        Some(VertexAttributeValues::Float32x3(colors)) if colors.len() == mesh.count_vertices()));
    assert_eq!(world.resource::<Assets<WaterMaterial>>().len(), 1);
    assert_eq!(
        world
            .query::<&MeshMaterial3d<WaterMaterial>>()
            .iter(world)
            .count(),
        1
    );
}

#[test]
fn first_water_geometry_after_a_dry_tile_enters_the_animation_queue() {
    for mode in [
        terra_core::ChunkAnimation::None,
        terra_core::ChunkAnimation::Fade,
    ] {
        let mut app = app();
        app.world_mut()
            .resource_mut::<GameConfig>()
            .client
            .chunk_animation = mode;
        let p = SectionPos::default();
        app.world_mut()
            .resource_mut::<EditJournal>()
            .record([1, 8, 1], 3)
            .unwrap();
        let mut dry = section(1);
        dry.water = None;
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(p, dry);
        app.update();
        assert_eq!(app.world().resource::<WaterStatus>().visible_tiles, 0);
        assert!(app.world().resource::<Assets<Mesh>>().is_empty());

        app.world_mut()
            .resource_mut::<EditJournal>()
            .record([1, 8, 1], 0)
            .unwrap();
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(p, section(2));
        app.update();
        assert_eq!(app.world().resource::<WaterStatus>().visible_tiles, 1);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        let world = app.world_mut();
        let root = world
            .query_filtered::<Entity, With<WaterSurface>>()
            .single(world)
            .unwrap();
        assert_eq!(
            world.get::<crate::transitions::QueuedChunk>(root).is_some(),
            mode != terra_core::ChunkAnimation::None
        );
        if mode != terra_core::ChunkAnimation::None {
            assert!(
                world.get::<Visibility>(root) == Some(&Visibility::Hidden)
                    || world
                        .get::<bevy::camera::visibility::RenderLayers>(root)
                        .is_some_and(|layers| {
                            !layers.intersects(&bevy::camera::visibility::RenderLayers::default())
                        }),
                "new water must not be visible to the main camera before admission"
            );
        }
    }
}

#[test]
fn water_upload_removes_meshes_on_dry_updates_and_eviction() {
    let mut app = app();
    let p = SectionPos::default();
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(p, section(0));
    app.update();
    assert_eq!(app.world().resource::<WaterStatus>().visible_tiles, 1);
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record([1, 8, 1], 3)
        .unwrap();
    let mut dry = section(1);
    dry.water = None;
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(p, dry);
    app.update();
    assert_eq!(app.world().resource::<WaterStatus>().visible_tiles, 0);
    assert!(app.world().resource::<Assets<Mesh>>().is_empty());
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(p, section(0));
    // A stale epoch cannot bring old water back after the edit.
    app.update();
    assert!(app.world().resource::<Assets<Mesh>>().is_empty());
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(p, section(1));
    // Same epoch was already published as dry; production always increments it on edits.
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record([1, 8, 1], 0)
        .unwrap();
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(p, section(2));
    app.update();
    assert_eq!(app.world().resource::<WaterStatus>().visible_tiles, 1);
    app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
    app.update();
    assert!(app.world().resource::<Assets<Mesh>>().is_empty());
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<WaterSurface>>()
            .iter(app.world())
            .count(),
        0
    );
}
