use super::builder::compose_section;
use crate::blocks::{BlockTemplate, DetailTemplate, PrimitiveTemplate, TemplateSet};
use terra_voxel::{SectionPos, meshing::BlockFace};

#[test]
fn adjacent_partial_segments_render_once_even_without_exposed_cube_interfaces() {
    use super::builder::{RenderOptions, compose_section_with_segments};
    let mut templates = templates();
    let model = templates.blocks[&1].clone();
    templates.blocks.insert(50, model);
    let cube =
        terra_voxel::meshing::mesh_section(&[50; terra_voxel::meshing::PADDED_VOLUME]).unwrap();
    assert!(cube.faces.is_empty());
    let a = terra_voxel::meshing::BlockSegment {
        material: 50,
        cell: [3, 4, 5],
        below: 0,
        above: 0,
        hanging: false,
        joined: false,
        fan: [0; 2],
    };
    let b = terra_voxel::meshing::BlockSegment {
        cell: [4, 4, 5],
        ..a
    };
    let options = RenderOptions {
        water_level: None,
        detail_level: super::DetailLevel::Near,
        terrain: None,
        exposure: None,
        tints: Default::default(),
        conditions: Default::default(),
    };
    let compose = |segments: &[terra_voxel::meshing::BlockSegment]| {
        compose_section_with_segments(
            &[],
            &templates,
            SectionPos::default(),
            &options,
            &[],
            segments,
        )
        .unwrap()
    };
    let once = compose(&[a]);
    let adjacent = compose(&[a, b, a]);
    assert_eq!(once.len(), 1);
    assert!(once[0].positions.len() < once[0].indices.len());
    assert_eq!(once[0].indices.len(), 96 * 3);
    assert_eq!(once[0].rooted_wind.len(), once[0].positions.len());
    let mut expected = Vec::new();
    terra_world::segments::visit(50, [3, 4, 5], Default::default(), false, |t, _| {
        expected.extend(t.map(|p| std::array::from_fn::<_, 3, _>(|i| p[i] + a.cell[i] as f32)));
    });
    assert_eq!(
        once[0]
            .indices
            .iter()
            .map(|&i| once[0].positions[i as usize])
            .collect::<Vec<_>>(),
        expected,
        "index sharing must preserve the canonical picking triangles"
    );
    assert_eq!(adjacent[0].positions.len(), once[0].positions.len() * 2);
    assert!(
        adjacent[0]
            .indices
            .iter()
            .all(|&i| (i as usize) < adjacent[0].positions.len())
    );
    assert_eq!(
        compose(&[b])[0].positions.len(),
        once[0].positions.len(),
        "removing one cell must retain the neighbor"
    );
}

#[test]
fn replacing_fixture_sections_cancels_old_snapshots_even_when_edit_epoch_is_unchanged() {
    use bevy::prelude::*;
    let mut world = World::new();
    world.init_resource::<super::GeometryCache>();
    world.init_resource::<super::RenderJobs>();
    let a = SectionPos::default();
    let b = SectionPos::new(-1, 0, 0);
    for p in [a, b] {
        world
            .resource_mut::<super::GeometryCache>()
            .insert_test(p, 0, Default::default());
    }
    let pool = bevy::tasks::TaskPoolBuilder::new().num_threads(1).build();
    world.resource_mut::<super::RenderJobs>().0.insert(
        a,
        super::jobs::RenderJob {
            ready: None,
            micro_detail: true,
            epoch: 0,
            appearance_epoch: 0,
            level: super::DetailLevel::Near,
            task: pool.spawn(std::future::pending()),
        },
    );
    super::forget_section_snapshots(&mut world, &[a]);
    assert!(!world.resource::<super::RenderJobs>().0.contains_key(&a));
    assert!(
        world
            .resource::<super::GeometryCache>()
            .snapshot(a, 0)
            .is_none()
    );
    assert!(
        world
            .resource::<super::GeometryCache>()
            .snapshot(b, 0)
            .is_some()
    );
}

fn templates() -> TemplateSet {
    let primitive = PrimitiveTemplate {
        tangents: Vec::new(),
        positions: vec![[0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 1.0]],
        normals: vec![[0.0, 1.0, 0.0]; 3],
        uvs: vec![[0.0, 0.0]; 3],
        indices: vec![0, 1, 2],
        material: 0,
    };
    TemplateSet {
        normal_mapped_materials: Default::default(),
        foliage_clusters: Default::default(),
        lantern_variants: Default::default(),
        opaque_cube_lods: Default::default(),
        animated_chests: false,
        cube_lods: Default::default(),
        crusher_moving_materials: Default::default(),
        train_moving_materials: Default::default(),
        conveyor_ribbon_materials: Default::default(),
        logistics_arrow_materials: Default::default(),
        logistics_support_materials: Default::default(),
        campfire_support_material: None,
        block_details: Default::default(),
        blocks: [(
            1,
            BlockTemplate {
                surface: None,
                model: Vec::new(),
                render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
                face_materials: [0; 6],
                side_cap: None,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| vec![primitive.clone()]),
                details: vec![DetailTemplate {
                    primitives: vec![primitive],
                    probability: 0.5,
                    water_clearance: None,
                    density_field: None,
                }],
            },
        )]
        .into(),
        prop_materials: Default::default(),
        item_materials: Default::default(),
        torch_palette: Default::default(),
        frame_materials: Default::default(),
        ore_materials: Default::default(),
        wood_materials: Default::default(),
        blanket_materials: Default::default(),
        hinge_materials: Default::default(),
        castle_cloth_materials: Default::default(),
        castle_icon_materials: Default::default(),
        castle_arch_join_materials: Default::default(),
        castle_decor_materials: Default::default(),
    }
}

#[test]
fn coastal_wash_is_one_batched_film_with_valid_indices_and_distinct_water_metadata() {
    use super::builder::{RenderOptions, compose_section_with_options};
    use std::sync::Arc;
    let mut templates = templates();
    let mut water = templates.blocks[&1].clone();
    water.face_materials = [1; 6];
    templates.blocks.insert(14, water);
    let mut exposure = crate::weather::surfaces::exposure::Exposure {
        natural: None,
        roofs: [6; 256],
        snow_roofs: [6; 256],
        coast: [0.0; 256],
        coast_vertices: [0.7; 289],
        water: [f32::NEG_INFINITY; 256],
    };
    exposure.coast[0] = 0.7;
    exposure.coast[1] = 0.3;
    let options = RenderOptions {
        water_level: Some(7.0),
        detail_level: super::DetailLevel::Near,
        terrain: None,
        exposure: Some(Arc::new(exposure)),
        tints: Default::default(),
        conditions: Default::default(),
    };
    let batches =
        compose_section_with_options(&[], &templates, SectionPos::new(-1, 0, -1), &options)
            .unwrap();
    assert_eq!(batches.len(), 1);
    let batch = &batches[0];
    assert_eq!(batch.positions.len(), 32);
    assert_eq!(batch.indices.len(), 48);
    assert!(
        batch
            .indices
            .iter()
            .all(|&i| (i as usize) < batch.positions.len())
    );
    assert!(batch.colors.iter().all(|v| v[3] == 6.0));
    assert!(batch.positions.iter().all(|v| (v[1] - 7.018).abs() < 0.001));
    assert!((batch.uvs[0][1] - 0.7).abs() < 0.001);
    let other_section =
        compose_section_with_options(&[], &templates, SectionPos::new(-1, 1, -1), &options)
            .unwrap();
    assert!(other_section.is_empty());
}

#[test]
fn dry_exposed_faces_carry_rain_exposure_without_whole_block_wet_tint() {
    use super::builder::{RenderOptions, compose_section_with_options};
    let mut templates = templates();
    templates.blocks.get_mut(&1).unwrap().details.clear();
    let exposure = crate::weather::surfaces::exposure::Exposure {
        natural: None,
        roofs: [4; 256],
        snow_roofs: [4; 256],
        coast: [0.0; 256],
        coast_vertices: [0.0; 289],
        water: [f32::NEG_INFINITY; 256],
    };
    let face = BlockFace {
        edge_mask: 15,
        occupied_neighbor: false,
        minimum: [0, 4, 0],
        width: 1,
        height: 1,
        u_axis: 0,
        v_axis: 2,
        side: 3,
        material: 1,
    };
    let options = RenderOptions {
        water_level: None,
        detail_level: super::DetailLevel::Near,
        terrain: None,
        exposure: Some(std::sync::Arc::new(exposure)),
        tints: Default::default(),
        conditions: Default::default(),
    };
    let batches =
        compose_section_with_options(&[face], &templates, SectionPos::default(), &options).unwrap();
    assert!(batches[0].surface_water.iter().all(|v| *v == [0.0, 1.0]));
    assert_eq!(batches[0].surface_water.len(), batches[0].positions.len());
    assert!(batches[0].colors.iter().all(|v| *v == [1.0; 4]));
}

#[test]
fn expands_rectangles_into_unit_template_faces() {
    let mut templates = templates();
    templates.blocks.get_mut(&1).expect("grass").details.clear();
    let face = BlockFace {
        edge_mask: 15,
        occupied_neighbor: false,
        minimum: [2, 3, 4],
        width: 2,
        height: 3,
        u_axis: 0,
        v_axis: 2,
        side: 3,
        material: 1,
    };
    let batches = compose_section(&[face], &templates, SectionPos { x: -1, y: 0, z: -1 }, None)
        .expect("compose");
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].positions.len(), 18);
    assert_eq!(batches[0].indices, (0..18).collect::<Vec<_>>());
    assert_eq!(batches[0].positions[0], [2.0, 4.0, 4.0]);
    assert_eq!(batches[0].positions[15], [3.0, 4.0, 6.0]);
}

#[test]
fn decorations_are_deterministic_and_missing_material_is_an_error() {
    let face = BlockFace {
        edge_mask: 15,
        occupied_neighbor: false,
        minimum: [0; 3],
        width: 16,
        height: 16,
        u_axis: 0,
        v_axis: 2,
        side: 3,
        material: 1,
    };
    let section = SectionPos { x: -1, y: 0, z: -1 };
    let templates = templates();
    let first = compose_section(&[face], &templates, section, None).expect("compose");
    assert_eq!(
        first,
        compose_section(&[face], &templates, section, None).expect("repeat")
    );
    assert!(first[0].positions.len() > 256 * 3);
    let missing = BlockFace {
        edge_mask: 15,
        occupied_neighbor: false,
        material: 65535,
        ..face
    };
    assert!(compose_section(&[missing], &templates, section, None).is_err());
}

#[test]
fn detailed_trunks_surrounded_by_masked_crowns_have_no_open_faces() {
    use crate::blocks::{
        cube_faces,
        definitions::{BlockRenderKind, CubeSurface},
        surface::SurfaceTemplate,
    };
    use bevy::math::Vec3;
    let mut templates = TemplateSet::default();
    for id in [6, 7, 8, 9, 10] {
        let bark = id == 6 || id == 10;
        templates.blocks.insert(
            id,
            BlockTemplate {
                surface: bark.then(|| SurfaceTemplate::new(CubeSurface::Wood)),
                model: Vec::new(),
                render_kind: BlockRenderKind::Cube,
                face_materials: [usize::from(id); 6],
                side_cap: None,
                bevel_width: if bark { 0.09 } else { 0.22 },
                faces: cube_faces(if bark { 0.09 } else { 0.22 }, [usize::from(id); 6], None),
                details: Vec::new(),
            },
        );
    }
    let index = |p: [usize; 3]| p[0] + 18 * (p[1] + 18 * p[2]);
    for leaf in 7..=9 {
        let mut padded = vec![0; terra_voxel::meshing::PADDED_VOLUME];
        for x in 3..=6 {
            for y in 3..=5 {
                for z in 3..=5 {
                    padded[index([x, y, z])] = leaf;
                }
            }
        }
        padded[index([4, 4, 4])] = 6;
        padded[index([5, 4, 4])] = 10;
        let mesh = terra_voxel::meshing::mesh_section(&padded).unwrap();
        for level in [
            super::DetailLevel::Near,
            super::DetailLevel::Middle,
            super::DetailLevel::Far,
        ] {
            let batches = super::builder::compose_section_with_options(
                &mesh.faces,
                &templates,
                SectionPos::new(-1, 0, -1),
                &super::builder::RenderOptions {
                    water_level: None,
                    detail_level: level,
                    terrain: None,
                    exposure: None,
                    tints: Default::default(),
                    conditions: Default::default(),
                },
            )
            .unwrap();
            let triangles: Vec<_> = batches
                .iter()
                .filter(|b| b.material == 6 || b.material == 10)
                .flat_map(|b| {
                    b.indices.chunks_exact(3).map(|t| {
                        t.iter()
                            .map(|&i| Vec3::from_array(b.positions[i as usize]))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            for block_x in [3.0, 4.0] {
                for axis in 0..3 {
                    for sign in [-1.0, 1.0] {
                        for u in [0.15, 0.35, 0.55, 0.75, 0.85] {
                            for v in [0.15, 0.35, 0.55, 0.75, 0.85] {
                                let mut center = Vec3::new(block_x, 3.0, 3.0);
                                center[axis] += 0.5;
                                center[(axis + 1) % 3] += u;
                                center[(axis + 2) % 3] += v;
                                let mut direction = Vec3::ZERO;
                                direction[axis] = -sign;
                                let origin = center - direction * 3.0;
                                let hit = triangles.iter().any(|t| {
                                    let e1 = t[1] - t[0];
                                    let e2 = t[2] - t[0];
                                    let p = direction.cross(e2);
                                    let det = e1.dot(p);
                                    if det <= 1e-7 {
                                        return false;
                                    }
                                    let relative = origin - t[0];
                                    let a = relative.dot(p) / det;
                                    let q = relative.cross(e1);
                                    let b = direction.dot(q) / det;
                                    let distance = e2.dot(q) / det;
                                    a >= -1e-5 && b >= -1e-5 && a + b <= 1.00001 && distance >= 0.0
                                });
                                assert!(
                                    hit,
                                    "Missing trunk face: leaf={leaf}, lod={level:?}, center={center:?}, axis={axis}, sign={sign}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn mesh_jobs_cannot_upload_after_a_newer_edit() {
    use super::{
        RenderStatus, RenderedSections,
        jobs::{RenderJob, RenderJobs},
        upload::upload_sections,
    };
    use crate::blocks::BlockAssets;
    use bevy::{prelude::*, tasks::AsyncComputeTaskPool};
    use terra_voxel::BlockSection;
    use terra_world::{
        edits::EditJournal,
        streaming::{ChunkStore, LoadedSection},
    };
    let position = SectionPos { x: 0, y: 0, z: 0 };
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<Assets<Mesh>>()
        .init_resource::<BlockAssets>()
        .init_resource::<RenderedSections>()
        .init_resource::<RenderStatus>()
        .init_resource::<EditJournal>()
        .init_resource::<ChunkStore>()
        .init_resource::<RenderJobs>()
        .add_systems(Update, upload_sections);
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        position,
        LoadedSection {
            props: Vec::new(),
            data: BlockSection::empty(position).expect("section"),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
        },
    );
    let task = AsyncComputeTaskPool::get()
        .spawn(async { (Ok(vec![super::builder::MeshBatch::default()]), 0.0) });
    app.world_mut().resource_mut::<RenderJobs>().0.insert(
        position,
        RenderJob {
            ready: None,
            micro_detail: true,
            epoch: 0,
            appearance_epoch: 0,
            level: super::DetailLevel::Near,
            task,
        },
    );
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record([1; 3], 3)
        .expect("new edit");
    for _ in 0..100 {
        app.update();
        if app.world().resource::<RenderJobs>().0.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(app.world().resource::<RenderJobs>().0.is_empty());
    assert!(
        app.world()
            .resource::<RenderedSections>()
            .sections
            .is_empty()
    );
    assert!(app.world().resource::<Assets<Mesh>>().is_empty());
}

#[test]
fn uploaded_glass_and_torch_models_never_cast_shadows_but_terrain_does() {
    use super::{
        RenderStatus, RenderedSections,
        jobs::{RenderJob, RenderJobs},
        upload::upload_sections,
    };
    use crate::blocks::BlockAssets;
    use bevy::{prelude::*, tasks::AsyncComputeTaskPool};
    use terra_voxel::BlockSection;
    use terra_world::{
        edits::EditJournal,
        streaming::{ChunkStore, LoadedSection},
    };
    for level in [
        super::DetailLevel::Near,
        super::DetailLevel::Middle,
        super::DetailLevel::Far,
    ] {
        let position = SectionPos::new(-1, 2, -1);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<BlockAssets>()
            .init_resource::<RenderedSections>()
            .init_resource::<RenderStatus>()
            .init_resource::<EditJournal>()
            .init_resource::<ChunkStore>()
            .init_resource::<RenderJobs>()
            .add_systems(Update, upload_sections);
        let mut library = templates();
        let mut glass = library.blocks[&1].clone();
        glass.face_materials = [1; 6];
        library.blocks.insert(15, glass);
        library.frame_materials.insert(15, 2);
        let mut torch = library.blocks[&1].clone();
        let mut primitive = torch.faces[0][0].clone();
        primitive.material = 3;
        torch.model = vec![primitive];
        library.blocks.insert(40, torch);
        let handles: Vec<_> = (0..4)
            .map(|_| {
                app.world_mut()
                    .resource_mut::<Assets<StandardMaterial>>()
                    .add(StandardMaterial::default())
            })
            .collect();
        {
            let mut assets = app.world_mut().resource_mut::<BlockAssets>();
            assets.templates = Some(std::sync::Arc::new(library));
            assets.materials = handles.clone();
        }
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            position,
            LoadedSection {
                props: Vec::new(),
                data: BlockSection::empty(position).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
            },
        );
        let task = AsyncComputeTaskPool::get().spawn(async {
            let batches = (0..4)
                .map(|material| super::builder::MeshBatch {
                    material,
                    positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                    normals: vec![[0.0, 0.0, 1.0]; 3],
                    uvs: vec![[0.0; 2]; 3],
                    indices: vec![0, 1, 2],
                    ..default()
                })
                .collect();
            (Ok(batches), 0.0)
        });
        app.world_mut().resource_mut::<RenderJobs>().0.insert(
            position,
            RenderJob {
                ready: None,
                micro_detail: true,
                epoch: 0,
                appearance_epoch: 0,
                level,
                task,
            },
        );
        for _ in 0..100 {
            app.update();
            if app.world().resource::<RenderJobs>().0.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let mut query = app.world_mut().query::<(
            &MeshMaterial3d<StandardMaterial>,
            Option<&bevy::light::NotShadowCaster>,
        )>();
        let batches: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(batches.len(), 4);
        for (material, no_shadow) in batches {
            assert_eq!(no_shadow.is_some(), material.0 != handles[0]);
        }
    }
}

#[test]
fn empty_terrain_batches_remove_previous_geometry_without_empty_gpu_allocations() {
    use super::{
        RenderStatus, RenderedSections,
        builder::MeshBatch,
        jobs::{RenderJob, RenderJobs},
        upload::upload_sections,
    };
    use crate::blocks::BlockAssets;
    use bevy::{prelude::*, tasks::AsyncComputeTaskPool};
    use terra_voxel::BlockSection;
    use terra_world::{
        edits::EditJournal,
        streaming::{ChunkStore, LoadedSection},
    };
    let position = SectionPos::new(-1, 0, -1);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<Assets<Mesh>>()
        .init_resource::<BlockAssets>()
        .init_resource::<RenderedSections>()
        .init_resource::<RenderStatus>()
        .init_resource::<EditJournal>()
        .init_resource::<ChunkStore>()
        .init_resource::<RenderJobs>()
        .add_systems(Update, upload_sections);
    app.world_mut()
        .resource_mut::<BlockAssets>()
        .materials
        .push(Handle::default());
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        position,
        LoadedSection {
            props: Vec::new(),
            data: BlockSection::empty(position).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
        },
    );
    for batches in [
        vec![MeshBatch {
            positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
            ..default()
        }],
        vec![
            MeshBatch::default(),
            MeshBatch {
                positions: vec![[0.0; 3]; 3],
                ..default()
            },
            MeshBatch {
                indices: vec![0, 1, 2],
                ..default()
            },
        ],
    ] {
        let expected = usize::from(batches.len() == 1);
        app.world_mut().resource_mut::<RenderJobs>().0.insert(
            position,
            RenderJob {
                ready: Some((Ok(batches), 0.0)),
                micro_detail: true,
                epoch: 0,
                appearance_epoch: 0,
                level: super::DetailLevel::Near,
                task: AsyncComputeTaskPool::get().spawn(std::future::pending()),
            },
        );
        app.update();
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), expected);
        assert_eq!(
            app.world().resource::<RenderedSections>().sections.len(),
            expected
        );
        let world = app.world_mut();
        assert_eq!(world.query::<&Mesh3d>().iter(world).count(), expected);
    }
}

#[test]
fn shore_vegetation_respects_water_clearance_and_never_spawns_submerged() {
    let mut templates = templates();
    let detail = &mut templates.blocks.get_mut(&1).unwrap().details[0];
    detail.probability = 1.0;
    detail.water_clearance = Some([0.05, 2.0]);
    let face = BlockFace {
        edge_mask: 15,
        occupied_neighbor: false,
        minimum: [0, 8, 0],
        width: 1,
        height: 1,
        u_axis: 0,
        v_axis: 2,
        side: 3,
        material: 1,
    };
    let compose = |water| {
        compose_section(&[face], &templates, SectionPos::default(), water).unwrap()[0]
            .positions
            .len()
    };
    assert_eq!(compose(Some(8.0)), 6);
    assert_eq!(compose(Some(9.0)), 3);
    assert_eq!(compose(Some(2.0)), 3);
    assert_eq!(compose(None), 3);
}

#[test]
fn repeated_terrain_lod_replacements_and_unloads_release_owned_meshes() {
    use super::{
        DetailLevel, RenderStatus, RenderedSections, TerrainSection,
        builder::MeshBatch,
        jobs::{RenderJob, RenderJobs},
        upload::upload_sections,
    };
    use crate::blocks::BlockAssets;
    use bevy::{prelude::*, tasks::AsyncComputeTaskPool};
    use terra_world::{
        edits::EditJournal,
        streaming::{ChunkStore, LoadedSection},
    };

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<BlockAssets>()
        .init_resource::<RenderedSections>()
        .init_resource::<RenderStatus>()
        .init_resource::<EditJournal>()
        .init_resource::<ChunkStore>()
        .init_resource::<RenderJobs>()
        .init_resource::<crate::LodSwapStatus>()
        .add_systems(Update, upload_sections);
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    app.world_mut()
        .resource_mut::<BlockAssets>()
        .materials
        .push(material.clone());
    app.update();
    let mut entities = app.world_mut().query::<Entity>();
    let baseline_entities = entities.iter(app.world()).count();
    let position = SectionPos::new(-1, -1, -1);
    for cycle in 0..100 {
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            position,
            LoadedSection {
                props: Vec::new(),
                data: terra_voxel::BlockSection::empty(position).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
            },
        );
        let mut previous = None;
        for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
            app.world_mut().resource_mut::<RenderJobs>().0.insert(
                position,
                RenderJob {
                    ready: Some((
                        Ok(vec![MeshBatch {
                            positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                            normals: vec![[0.0, 0.0, 1.0]; 3],
                            uvs: vec![[0.0; 2]; 3],
                            indices: vec![0, 1, 2],
                            ..default()
                        }]),
                        0.0,
                    )),
                    micro_detail: true,
                    epoch: 0,
                    appearance_epoch: 0,
                    level,
                    task: AsyncComputeTaskPool::get().spawn(std::future::pending()),
                },
            );
            app.update();
            let world = app.world_mut();
            assert_eq!(world.resource::<RenderedSections>().sections.len(), 1);
            assert_eq!(world.resource::<Assets<Mesh>>().len(), 1, "cycle {cycle}");
            assert_eq!(entities.iter(world).count(), baseline_entities + 2);
            assert_eq!(world.query::<&TerrainSection>().iter(world).count(), 1);
            assert_eq!(world.query::<&Mesh3d>().iter(world).count(), 1);
            if let Some(old) = previous.take() {
                assert!(world.resource::<Assets<Mesh>>().get(old).is_none());
            }
            previous =
                Some(world.resource::<RenderedSections>().sections[&position].meshes[0].id());
            assert_eq!(world.resource::<Assets<StandardMaterial>>().len(), 1);
            assert!(
                world
                    .resource::<Assets<StandardMaterial>>()
                    .contains(material.id())
            );
        }
        app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
        app.update();
        let world = app.world_mut();
        assert!(world.resource::<Assets<Mesh>>().is_empty());
        assert!(world.resource::<RenderedSections>().sections.is_empty());
        assert_eq!(
            entities.iter(world).count(),
            baseline_entities,
            "cycle {cycle}"
        );
        assert_eq!(world.query::<&Mesh3d>().iter(world).count(), 0);
        assert!(world.resource::<RenderJobs>().0.is_empty());
        assert_eq!(world.resource::<Assets<StandardMaterial>>().len(), 1);
    }
    assert_eq!(
        app.world().resource::<crate::LodSwapStatus>().completed,
        200
    );
}

#[test]
fn far_lod_uses_four_vertices_per_greedy_rectangle_and_correct_winding() {
    use super::DetailLevel;
    use super::builder::{RenderOptions, compose_section_with_options};
    use bevy::prelude::Vec3;
    let templates = templates();
    let options = RenderOptions {
        water_level: None,
        detail_level: DetailLevel::Far,
        terrain: None,
        exposure: None,
        tints: Default::default(),
        conditions: Default::default(),
    };
    for side in 0..6 {
        let axis = side / 2;
        let face = BlockFace {
            edge_mask: 15,
            occupied_neighbor: false,
            minimum: [0; 3],
            width: 16,
            height: 16,
            u_axis: (axis + 1) % 3,
            v_axis: (axis + 2) % 3,
            side,
            material: 1,
        };
        let batches =
            compose_section_with_options(&[face], &templates, SectionPos::default(), &options)
                .unwrap();
        assert_eq!(batches[0].positions.len(), 4);
        assert_eq!(batches[0].indices.len(), 6);
        assert_eq!(batches[0].uvs[2], [16.0, 16.0]);
        let b = &batches[0];
        let [a, c, d] = [b.indices[0], b.indices[1], b.indices[2]]
            .map(|i| Vec3::from_array(b.positions[i as usize]));
        assert!((c - a).cross(d - a).dot(Vec3::from_array(b.normals[0])) > 0.0);
    }
}

#[test]
fn lod_scan_focus_quantization_uses_floor_and_tracks_vertical_camera_motion() {
    use super::jobs::focus_bucket;
    assert_eq!(focus_bucket([-0.01, 400.0, -16.01]), [-1, 100, -5]);
    assert_eq!(focus_bucket([-0.99, -80.0, -16.99]), [-1, -20, -5]);
    assert_eq!(focus_bucket([-3.99, 403.99, -19.99]), [-1, 100, -5]);
    assert_ne!(focus_bucket([-4.01, 400.0, -16.01]), [-1, 100, -5]);
    assert_ne!(focus_bucket([-0.01, 404.0, -16.01]), [-1, 100, -5]);
}

#[test]
fn lod_selection_changes_with_camera_distance() {
    use super::{DetailLevel, lod::detail_level};
    let p = SectionPos::default();
    assert_eq!(detail_level(p, [8.0, 1000.0, 8.0]), DetailLevel::Near);
    assert_eq!(detail_level(p, [120.0, 0.0, 8.0]), DetailLevel::Near);
    assert_eq!(detail_level(p, [180.0, 0.0, 8.0]), DetailLevel::Middle);
    assert_eq!(detail_level(p, [288.0, 0.0, 8.0]), DetailLevel::Far);
    assert_eq!(detail_level(p, [8.0, 0.0, 8.0]), DetailLevel::Near);
}

#[test]
fn lod_hysteresis_is_stable_at_both_boundaries_and_after_teleports() {
    use super::{DetailLevel::*, lod::select_level};
    let config = terra_core::ClientConfig::default();
    for position in [SectionPos::default(), SectionPos::new(-10, -4, -20)] {
        let origin = position.origin().unwrap();
        let focus = |distance| {
            [
                origin[0] as f64 + 16.0 + distance * 1.5,
                1000.0,
                origin[2] as f64 + 8.0,
            ]
        };
        for distance in [95.0, 96.0, 97.0, 105.0] {
            assert_eq!(
                select_level(position, focus(distance), Some(Near), &config),
                Near
            );
            assert_eq!(
                select_level(position, focus(distance), Some(Middle), &config),
                Middle
            );
        }
        assert_eq!(
            select_level(position, focus(109.0), Some(Near), &config),
            Middle
        );
        for distance in [159.0, 160.0, 161.0, 170.0] {
            assert_eq!(
                select_level(position, focus(distance), Some(Middle), &config),
                Middle
            );
            assert_eq!(
                select_level(position, focus(distance), Some(Far), &config),
                Far
            );
        }
        assert_eq!(
            select_level(position, focus(200.0), Some(Near), &config),
            Far
        );
        assert_eq!(select_level(position, focus(0.0), Some(Far), &config), Near);
    }
}

#[test]
fn coarse_grass_preserves_top_and_side_cap_independently_of_primitive_order() {
    use super::{
        DetailLevel,
        builder::{RenderOptions, compose_section_with_options},
    };
    let mut templates = templates();
    let block = templates.blocks.get_mut(&1).unwrap();
    block.face_materials = [1, 1, 1, 2, 1, 1];
    block.side_cap = Some(crate::blocks::SideCap {
        material: 2,
        height: 0.12,
    });
    // The first imported primitive deliberately belongs to soil, not the grass cap.
    for face in &mut block.faces {
        face[0].material = 1;
    }
    for level in [DetailLevel::Middle, DetailLevel::Far] {
        let options = RenderOptions {
            detail_level: level,
            water_level: None,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: Default::default(),
        };
        let top = BlockFace {
            edge_mask: 15,
            occupied_neighbor: false,
            minimum: [0; 3],
            width: 16,
            height: 16,
            u_axis: 0,
            v_axis: 2,
            side: 3,
            material: 1,
        };
        let batches =
            compose_section_with_options(&[top], &templates, SectionPos::new(-1, 0, -1), &options)
                .unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].material, 2);
        for side in [0, 1, 4, 5] {
            let face = BlockFace {
                edge_mask: 15,
                occupied_neighbor: false,
                u_axis: if side < 2 { 2 } else { 0 },
                v_axis: 1,
                side,
                ..top
            };
            let batches =
                compose_section_with_options(&[face], &templates, SectionPos::default(), &options)
                    .unwrap();
            assert_eq!(batches.len(), 2);
            let cap = batches.iter().find(|b| b.material == 2).unwrap();
            assert_eq!(cap.positions.len(), 16 * 4);
            for quad in cap.positions.chunks_exact(4) {
                let low = quad.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
                let high = quad.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
                assert!((high - low - 0.12).abs() < 0.00001);
            }
        }
    }
}

#[test]
fn custom_mesh_keeps_every_authored_face_material_at_far_lod() {
    use super::{
        DetailLevel,
        builder::{RenderOptions, compose_section_with_options},
    };
    let mut templates = templates();
    let block = templates.blocks.get_mut(&1).unwrap();
    block.render_kind = crate::blocks::definitions::BlockRenderKind::CustomMesh;
    let mut secondary = block.faces[3][0].clone();
    secondary.material = 7;
    block.faces[3].push(secondary);
    let face = BlockFace {
        edge_mask: 15,
        occupied_neighbor: false,
        minimum: [0; 3],
        width: 1,
        height: 1,
        u_axis: 0,
        v_axis: 2,
        side: 3,
        material: 1,
    };
    let batches = compose_section_with_options(
        &[face],
        &templates,
        SectionPos::default(),
        &RenderOptions {
            detail_level: DetailLevel::Far,
            water_level: None,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: Default::default(),
        },
    )
    .unwrap();
    assert_eq!(
        batches.iter().map(|b| b.material).collect::<Vec<_>>(),
        vec![0, 7]
    );
}

#[test]
fn arrivals_preserve_pending_terrain_lod_replacements() {
    use super::{DetailLevel, GeometryCache, RenderJobs, RenderStatus, RenderedSections};
    use bevy::prelude::*;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    use terra_core::GameConfig;
    use terra_world::{
        TerraGenerator,
        edits::EditJournal,
        streaming::{ChunkFocus, ChunkStore, LoadedSection, WorldTerrain},
    };
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<crate::blocks::BlockAssets>()
        .init_resource::<RenderedSections>()
        .init_resource::<RenderStatus>()
        .init_resource::<RenderJobs>()
        .init_resource::<GeometryCache>()
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .init_resource::<GameConfig>()
        .insert_resource(ChunkFocus([8.0, 0.0, 8.0]))
        .insert_resource(WorldTerrain(
            TerraGenerator::new(Default::default()).unwrap(),
        ))
        .add_systems(
            Update,
            (super::upload_sections, super::dispatch_meshes).chain(),
        );
    crate::transitions::install(&mut app);
    app.world_mut()
        .resource_mut::<GameConfig>()
        .client
        .max_mesh_jobs = 1;
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    {
        let mut assets = app.world_mut().resource_mut::<crate::blocks::BlockAssets>();
        assets.materials.push(material);
        assets.templates = Some(Arc::new(templates()));
    }
    let section = |p, faces| LoadedSection {
        data: terra_voxel::BlockSection::empty(p).unwrap(),
        props: Vec::new(),
        mesh: Some(terra_voxel::meshing::BlockMesh { faces, ..default() }),
        mesh_epoch: 0,
        water_epoch: 0,
        water: None,
    };
    let owners: Vec<_> = (0..8).map(|x| SectionPos::new(x, 0, 0)).collect();
    for &p in &owners {
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            p,
            section(
                p,
                vec![BlockFace {
                    edge_mask: 15,
                    occupied_neighbor: false,
                    minimum: [0; 3],
                    width: 16,
                    height: 16,
                    u_axis: 0,
                    v_axis: 2,
                    side: 3,
                    material: 1,
                }],
            ),
        );
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.world().resource::<RenderedSections>().sections.len() < owners.len() {
        app.update();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    app.world_mut().resource_mut::<ChunkFocus>().0 = [1000.0, 0.0, 0.0];
    loop {
        let p = SectionPos::new(-20, 0, 0);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(p, section(p, Vec::new()));
        app.update();
        if owners.iter().all(|p| {
            app.world()
                .resource::<RenderedSections>()
                .sections
                .get(p)
                .is_some_and(|r| r.detail_level == DetailLevel::Far)
        }) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "streaming starved a queued terrain LOD"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn published_terrain_arrivals_survive_full_workers_and_message_expiry() {
    use super::{DetailLevel, GeometryCache, RenderJobs, RenderStatus, RenderedSections};
    use bevy::prelude::*;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    use terra_core::{ChunkAnimation, GameConfig};
    use terra_world::{
        TerraGenerator,
        edits::EditJournal,
        streaming::{
            ChunkFocus, ChunkStore, LoadedSection, PreparedColumnChanged, SectionPublished,
            SectionRemoved, WorldTerrain,
        },
    };
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<crate::blocks::BlockAssets>()
        .init_resource::<RenderedSections>()
        .init_resource::<RenderStatus>()
        .init_resource::<RenderJobs>()
        .init_resource::<GeometryCache>()
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .init_resource::<GameConfig>()
        .add_message::<SectionPublished>()
        .add_message::<SectionRemoved>()
        .add_message::<PreparedColumnChanged>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_secs_f64(1.0 / 60.0),
        ))
        .insert_resource(ChunkFocus([8.0, 160.0, 8.0]))
        .insert_resource(WorldTerrain(
            TerraGenerator::new(Default::default()).unwrap(),
        ))
        .add_systems(
            Update,
            (super::upload_sections, super::dispatch_meshes).chain(),
        );
    crate::transitions::install(&mut app);
    {
        let mut config = app.world_mut().resource_mut::<GameConfig>();
        config.client.chunk_animation = ChunkAnimation::None;
        config.client.max_mesh_jobs = 2;
    }
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    {
        let mut assets = app.world_mut().resource_mut::<crate::blocks::BlockAssets>();
        assets.materials.push(material);
        assets.templates = Some(Arc::new(templates()));
    }
    let section = |position| LoadedSection {
        data: terra_voxel::BlockSection::empty(position).unwrap(),
        props: Vec::new(),
        mesh: Some(terra_voxel::meshing::BlockMesh {
            faces: vec![BlockFace {
                edge_mask: 15,
                occupied_neighbor: false,
                minimum: [0; 3],
                width: 16,
                height: 16,
                u_axis: 0,
                v_axis: 2,
                side: 3,
                material: 1,
            }],
            ..default()
        }),
        mesh_epoch: 0,
        water_epoch: 0,
        water: None,
    };
    let owners: Vec<_> = (0..12).map(|x| SectionPos::new(x, 10, 0)).collect();
    for &position in &owners {
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(position, section(position));
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.world().resource::<RenderedSections>().sections.len() < owners.len()
        || !app.world().resource::<RenderJobs>().0.is_empty()
    {
        app.update();
        assert!(Instant::now() < deadline, "initial fixture did not settle");
        std::thread::sleep(Duration::from_millis(1));
    }
    let before: Vec<_> = owners
        .iter()
        .map(|&position| {
            let rendered = &app.world().resource::<RenderedSections>().sections[&position];
            (
                position,
                rendered.entity,
                rendered.meshes.iter().map(Handle::id).collect::<Vec<_>>(),
            )
        })
        .collect();
    for &position in &owners[..2] {
        app.world_mut().resource_mut::<RenderJobs>().0.insert(
            position,
            super::jobs::RenderJob {
                ready: None,
                micro_detail: true,
                epoch: 0,
                appearance_epoch: 0,
                level: DetailLevel::Near,
                task: bevy::tasks::AsyncComputeTaskPool::get().spawn(std::future::pending()),
            },
        );
    }
    let arrivals: Vec<_> = (20..25).map(|x| SectionPos::new(x, 10, 0)).collect();
    for &position in &arrivals {
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(position, section(position));
        app.world_mut()
            .resource_mut::<Messages<SectionPublished>>()
            .write(SectionPublished { position });
    }
    for _ in 0..4 {
        app.update();
    }
    assert!(
        app.world()
            .resource::<Messages<SectionPublished>>()
            .is_empty()
    );
    assert_eq!(
        app.world().resource::<RenderedSections>().sections.len(),
        owners.len()
    );
    for position in &owners[..2] {
        app.world_mut()
            .resource_mut::<RenderJobs>()
            .0
            .remove(position);
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.world().resource::<RenderedSections>().sections.len() < owners.len() + arrivals.len()
        || !app.world().resource::<RenderJobs>().0.is_empty()
    {
        app.update();
        assert!(
            Instant::now() < deadline,
            "published arrivals were lost after their messages expired"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let rendered = app.world().resource::<RenderedSections>();
    for (position, entity, meshes) in before {
        let current = &rendered.sections[&position];
        assert_eq!(current.entity, entity);
        assert_eq!(
            current.meshes.iter().map(Handle::id).collect::<Vec<_>>(),
            meshes
        );
    }
    assert!(arrivals.iter().all(|position| {
        app.world().resource::<ChunkStore>().loaded[position]
            .mesh
            .is_none()
    }));
}

#[test]
fn edited_section_becomes_visible_while_all_ordinary_composition_workers_are_blocked() {
    use super::DetailLevel;
    use bevy::prelude::*;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    use terra_world::{
        edits::EditJournal,
        streaming::{ChunkFocus, ChunkStore, LoadedSection, SectionPublished, WorldTerrain},
    };
    let mut config = terra_core::GameConfig::default();
    config.client.max_mesh_jobs = 1;
    config.client.chunk_animation = terra_core::ChunkAnimation::Fade;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(config)
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<crate::blocks::BlockAssets>()
        .init_resource::<super::RenderedSections>()
        .init_resource::<super::RenderStatus>()
        .init_resource::<super::jobs::RenderJobs>()
        .init_resource::<super::GeometryCache>()
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .add_message::<SectionPublished>()
        .insert_resource(ChunkFocus([8.0, 168.0, 8.0]))
        .insert_resource(WorldTerrain(
            terra_world::TerraGenerator::new(Default::default()).unwrap(),
        ))
        .add_systems(
            Update,
            (super::upload_sections, super::dispatch_meshes).chain(),
        );
    crate::transitions::install(&mut app);
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    {
        let mut assets = app.world_mut().resource_mut::<crate::blocks::BlockAssets>();
        assets.materials.push(material);
        assets.templates = Some(Arc::new(templates()));
    }
    let owner = SectionPos::new(0, 10, 0);
    let blocked = SectionPos::new(-1, 10, -1);
    // The empty section was already composed before the player placed a block.
    // A never-seen saved section must still use its first chunk entrance.
    app.world_mut()
        .resource_mut::<super::RenderedSections>()
        .block_revisions
        .insert(owner, 0);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record(owner.global_block([8, 8, 8]).unwrap(), 1)
        .unwrap();
    for position in [blocked, owner] {
        let mut data = terra_voxel::BlockSection::empty(position).unwrap();
        data.set_block([8, 8, 8], 1).unwrap();
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            position,
            LoadedSection {
                data,
                props: Vec::new(),
                water: None,
                water_epoch: 0,
                mesh_epoch: u64::from(position == owner),
                mesh: (position == owner).then(|| terra_voxel::meshing::BlockMesh {
                    faces: vec![BlockFace {
                        minimum: [8, 8, 8],
                        width: 1,
                        height: 1,
                        u_axis: 0,
                        v_axis: 2,
                        side: 3,
                        material: 1,
                        occupied_neighbor: false,
                        edge_mask: 15,
                    }],
                    ..default()
                }),
            },
        );
    }
    app.world_mut()
        .resource_mut::<super::jobs::RenderJobs>()
        .0
        .insert(
            blocked,
            super::jobs::RenderJob {
                ready: None,
                micro_detail: true,
                epoch: 0,
                appearance_epoch: 0,
                level: DetailLevel::Near,
                task: bevy::tasks::AsyncComputeTaskPool::get().spawn(std::future::pending()),
            },
        );
    app.world_mut()
        .write_message(SectionPublished { position: owner });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        let jobs = app.world().resource::<super::jobs::RenderJobs>();
        assert!(
            jobs.0.len() <= 2,
            "one bounded interactive slot, never unbounded parallel jobs"
        );
        assert!(
            jobs.0.contains_key(&blocked),
            "the unrelated job must still be blocked"
        );
        if let Some(section) = app
            .world()
            .resource::<super::RenderedSections>()
            .sections
            .get(&owner)
        {
            assert_eq!(section.mesh_epoch, 1);
            assert!(!section.meshes.is_empty());
            assert!(
                app.world()
                    .get::<crate::transitions::QueuedChunk>(section.entity)
                    .is_none(),
                "first placed block is not another generated chunk entrance"
            );
            assert!(
                app.world()
                    .get::<crate::transitions::ChunkFade>(section.entity)
                    .is_none()
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "a confirmed placement waited for an unrelated composition worker"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn terrain_selection_retries_only_finished_roots_after_entry() {
    use super::{
        DetailLevel, GeometryCache, RenderJobs, RenderStatus, RenderedSections, TerrainSection,
    };
    use bevy::prelude::*;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    use terra_core::{ChunkAnimation, GameConfig};
    use terra_world::{
        TerraGenerator,
        edits::EditJournal,
        streaming::{ChunkFocus, ChunkStore, LoadedSection, WorldTerrain},
    };
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<crate::blocks::BlockAssets>()
        .init_resource::<RenderedSections>()
        .init_resource::<RenderStatus>()
        .init_resource::<RenderJobs>()
        .init_resource::<GeometryCache>()
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .init_resource::<GameConfig>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_secs_f32(1.0 / 30.0),
        ))
        .insert_resource(ChunkFocus([8.0, 0.0, 8.0]))
        .insert_resource(WorldTerrain(
            TerraGenerator::new(Default::default()).unwrap(),
        ))
        .add_systems(
            Update,
            (super::upload_sections, super::dispatch_meshes).chain(),
        );
    crate::transitions::install(&mut app);
    {
        let mut config = app.world_mut().resource_mut::<GameConfig>();
        config.client.chunk_animation = ChunkAnimation::Fade;
        config.client.max_mesh_jobs = 2;
    }
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    {
        let mut assets = app.world_mut().resource_mut::<crate::blocks::BlockAssets>();
        assets.materials.push(material);
        assets.templates = Some(Arc::new(templates()));
    }
    let owner = SectionPos::default();
    let stable = SectionPos::new(100, 0, 0);
    for position in [owner, stable] {
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            position,
            LoadedSection {
                data: terra_voxel::BlockSection::empty(position).unwrap(),
                props: Vec::new(),
                mesh: Some(terra_voxel::meshing::BlockMesh {
                    faces: vec![BlockFace {
                        edge_mask: 15,
                        occupied_neighbor: false,
                        minimum: [0; 3],
                        width: 16,
                        height: 16,
                        u_axis: 0,
                        v_axis: 2,
                        side: 3,
                        material: 1,
                    }],
                    ..default()
                }),
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
            },
        );
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        let rendered = app.world().resource::<RenderedSections>();
        if [owner, stable].into_iter().all(|position| {
            rendered.sections.get(&position).is_some_and(|section| {
                app.world()
                    .get::<crate::transitions::ChunkFade>(section.entity)
                    .is_some()
            })
        }) && app.world().resource::<RenderJobs>().0.is_empty()
        {
            break;
        }
        assert!(Instant::now() < deadline, "entry fades did not start");
        std::thread::sleep(Duration::from_millis(1));
    }
    let owner_root = app.world().resource::<RenderedSections>().sections[&owner].entity;
    let stable_root = app.world().resource::<RenderedSections>().sections[&stable].entity;
    app.world_mut().resource_mut::<ChunkFocus>().0 = [312.0, 0.0, 8.0];
    app.update();
    assert_eq!(
        app.world().resource::<RenderedSections>().sections[&owner].detail_level,
        DetailLevel::Near
    );
    assert!(app.world().resource::<RenderJobs>().0.is_empty());
    assert!(
        app.world()
            .get::<crate::transitions::ChunkFade>(owner_root)
            .is_some()
    );
    let unrelated = app
        .world_mut()
        .spawn(crate::transitions::EnteringChunk)
        .id();
    let retired = app
        .world_mut()
        .spawn((
            TerrainSection {
                position: owner,
                mesh_epoch: 0,
                detail_level: DetailLevel::Near,
            },
            crate::transitions::EnteringChunk,
        ))
        .id();
    for entity in [unrelated, retired] {
        app.world_mut()
            .entity_mut(entity)
            .remove::<crate::transitions::EnteringChunk>();
    }
    app.update();
    assert!(app.world().resource::<RenderJobs>().0.is_empty());
    assert_eq!(
        app.world().resource::<RenderedSections>().sections[&owner].entity,
        owner_root
    );
    let feedback = app
        .world()
        .resource::<crate::transitions::TransitionDraws>();
    let drawn: Vec<_> = feedback.pending.iter().copied().collect();
    assert!(!drawn.is_empty());
    for entity in drawn {
        feedback.drawn(entity);
    }
    for _ in 0..12 {
        app.update();
    }
    assert!(
        app.world()
            .get::<crate::transitions::EnteringChunk>(owner_root)
            .is_none()
    );
    loop {
        app.update();
        let rendered = app.world().resource::<RenderedSections>();
        if rendered.sections[&owner].detail_level == DetailLevel::Far
            && app.world().resource::<RenderJobs>().0.is_empty()
        {
            assert_ne!(rendered.sections[&owner].entity, owner_root);
            assert_eq!(rendered.sections[&stable].entity, stable_root);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "a finished entry lost its stationary LOD retry"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn render_lod_rebuilds_from_cached_faces_and_eviction_frees_every_mesh() {
    use super::{
        DetailLevel, GeometryCache, RenderJobs, RenderStatus, RenderedSections, dispatch_meshes,
        upload_sections,
    };
    use crate::blocks::BlockAssets;
    use bevy::{asset::RenderAssetUsages, camera::primitives::MeshAabb, prelude::*};
    use std::{sync::Arc, time::Duration};
    use terra_core::GameConfig;
    use terra_world::{
        TerraGenerator,
        edits::EditJournal,
        streaming::{ChunkFocus, ChunkStore, LoadedSection, WorldTerrain},
    };
    #[derive(Resource, Default)]
    struct WorldChanges(usize);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<BlockAssets>()
        .init_resource::<RenderedSections>()
        .init_resource::<RenderStatus>()
        .init_resource::<RenderJobs>()
        .init_resource::<GeometryCache>()
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .init_resource::<GameConfig>()
        .insert_resource(ChunkFocus([8.0, 0.0, 8.0]))
        .insert_resource(WorldTerrain(
            TerraGenerator::new(Default::default()).unwrap(),
        ))
        .add_systems(Update, (upload_sections, dispatch_meshes).chain())
        .init_resource::<WorldChanges>()
        .add_systems(
            Last,
            |chunks: Res<ChunkStore>, mut changes: ResMut<WorldChanges>| {
                changes.0 += usize::from(chunks.is_changed());
            },
        );
    crate::transitions::install(&mut app);
    app.world_mut()
        .resource_mut::<GameConfig>()
        .client
        .chunk_animation = terra_core::ChunkAnimation::Fade;
    app.world_mut().spawn((
        terra_player::TerraCamera,
        bevy::camera::primitives::Frustum::default(),
        GlobalTransform::default(),
    ));
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    {
        let mut assets = app.world_mut().resource_mut::<BlockAssets>();
        assets.materials.push(material);
        assets.templates = Some(Arc::new(templates()));
    }
    let p = SectionPos::default();
    let face = BlockFace {
        edge_mask: 15,
        occupied_neighbor: false,
        minimum: [0; 3],
        width: 16,
        height: 16,
        u_axis: 0,
        v_axis: 2,
        side: 3,
        material: 1,
    };
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        p,
        LoadedSection {
            props: Vec::new(),
            data: terra_voxel::BlockSection::empty(p).unwrap(),
            mesh: Some(terra_voxel::meshing::BlockMesh {
                faces: vec![face],
                ..default()
            }),
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
        },
    );
    let settle = |app: &mut App, level, queued| {
        for _ in 0..500 {
            app.update();
            if !app
                .world()
                .resource::<RenderedSections>()
                .sections
                .is_empty()
            {
                let world = app.world_mut();
                assert_eq!(
                    world.query::<&super::TerrainSection>().iter(world).count(),
                    1
                );
                let current = &world.resource::<RenderedSections>().sections[&p];
                let near = current.detail_level == DetailLevel::Near;
                assert_eq!(current.meshes.len(), 1 + usize::from(near));
                assert_eq!(world.resource::<Assets<Mesh>>().len(), current.meshes.len());
                let root = current.entity;
                if current.detail_level == level
                    && current.appearance_epoch
                        == world.resource::<EditJournal>().appearance_epoch(p)
                {
                    assert_eq!(
                        world.get::<crate::transitions::QueuedChunk>(root).is_some(),
                        queued,
                        "Unrevealed replacements retain their initial hidden state"
                    );
                    assert_eq!(
                        world.get::<Visibility>(root),
                        Some(&if queued {
                            Visibility::Hidden
                        } else {
                            Visibility::Inherited
                        })
                    );
                }
                assert_eq!(
                    world
                        .query::<&super::shadows::ShadowProxy>()
                        .iter(world)
                        .count(),
                    usize::from(near)
                );
                assert_eq!(
                    world
                        .query::<&bevy::camera::visibility::VisibilityRange>()
                        .iter(world)
                        .count(),
                    0,
                    "opaque terrain must never be dithered during LOD replacement"
                );
            }
            if app
                .world()
                .resource::<RenderedSections>()
                .sections
                .get(&p)
                .is_some_and(|r| r.detail_level == level)
                && app.world().resource::<RenderJobs>().0.is_empty()
            {
                return;
            }
            assert!(app.world().resource::<RenderStatus>().error.is_none());
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("LOD update timed out");
    };
    settle(&mut app, DetailLevel::Near, true);
    assert_eq!(app.world().resource::<GeometryCache>().len(), 1);
    assert_eq!(app.world().resource::<WorldChanges>().0, 1);
    assert_eq!(app.world().resource::<crate::LodSwapStatus>().completed, 0);
    // A nearby stale snapshot awaiting generation cannot consume the only
    // dispatch slot and starve a valid distant LOD replacement.
    app.world_mut()
        .resource_mut::<GameConfig>()
        .client
        .max_mesh_jobs = 1;
    let stale = SectionPos::new(19, 0, 0);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record([304, 0, 0], 3)
        .unwrap();
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        stale,
        LoadedSection {
            props: Vec::new(),
            data: terra_voxel::BlockSection::empty(stale).unwrap(),
            mesh: Some(terra_voxel::meshing::BlockMesh {
                faces: vec![face],
                ..default()
            }),
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
        },
    );
    app.world_mut().resource_mut::<ChunkFocus>().0 = [312.0, 0.0, 8.0];
    settle(&mut app, DetailLevel::Far, true);
    assert_eq!(
        app.world().resource::<WorldChanges>().0,
        2,
        "a LOD swap must not invalidate world, water or weather caches"
    );
    assert_eq!(app.world().resource::<crate::LodSwapStatus>().completed, 1);
    let handle = app.world().resource::<RenderedSections>().sections[&p].meshes[0].clone();
    assert_eq!(
        app.world()
            .resource::<Assets<Mesh>>()
            .get(&handle)
            .unwrap()
            .count_vertices(),
        4
    );
    {
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        let mut mesh = meshes.get_mut(&handle).unwrap();
        assert_eq!(mesh.asset_usage, RenderAssetUsages::RENDER_WORLD);
        let before = mesh.get_aabb().unwrap();
        let gpu_data = mesh.take_gpu_data().unwrap();
        assert_eq!(gpu_data.count_vertices(), 4);
        assert_eq!(gpu_data.asset_usage, RenderAssetUsages::RENDER_WORLD);
        assert!(mesh.try_attribute(Mesh::ATTRIBUTE_POSITION).is_err());
        assert!(mesh.final_aabb.is_some());
        let after = mesh.get_aabb().unwrap();
        assert_eq!(after.center, before.center);
        assert_eq!(after.half_extents, before.half_extents);
    }
    app.world_mut().resource_mut::<ChunkFocus>().0 = [8.0, 0.0, 8.0];
    settle(&mut app, DetailLevel::Near, true);
    assert_eq!(app.world().resource::<crate::LodSwapStatus>().completed, 2);
    let old_entity = app.world().resource::<RenderedSections>().sections[&p].entity;
    let old_appearance = app.world().resource::<RenderedSections>().sections[&p].appearance_epoch;
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_surface_condition(
            [1, 0, 1],
            terra_world::environment::SurfaceCondition::new(0.0, 1.0).unwrap(),
        )
        .unwrap();
    settle(&mut app, DetailLevel::Near, true);
    let updated = &app.world().resource::<RenderedSections>().sections[&p];
    assert_eq!(updated.appearance_epoch, old_appearance.wrapping_add(1));
    assert_eq!(updated.mesh_epoch, 0);
    assert_ne!(updated.entity, old_entity);
    assert_eq!(app.world().resource::<crate::LodSwapStatus>().completed, 2);
    assert!(
        app.world().resource::<ChunkStore>().loaded[&p]
            .mesh
            .is_none()
    );
    // A result can become obsolete while its position was excluded from the
    // cached candidate list. It must be retried even after the camera stops.
    app.world_mut().resource_mut::<RenderJobs>().0.insert(
        p,
        super::jobs::RenderJob {
            ready: Some((Ok(Vec::new()), 0.0)),
            micro_detail: false,
            epoch: 0,
            appearance_epoch: 0,
            level: DetailLevel::Far,
            task: bevy::tasks::AsyncComputeTaskPool::get().spawn(std::future::pending()),
        },
    );
    app.world_mut().resource_mut::<RenderStatus>().ready = true;
    settle(&mut app, DetailLevel::Near, true);
    assert!(app.world().resource::<RenderJobs>().1.is_empty());
    assert!(!app.world().resource::<Assets<Mesh>>().is_empty());
    app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
    app.update();
    assert_eq!(app.world().resource::<GeometryCache>().len(), 0);
    assert!(app.world().resource::<Assets<Mesh>>().is_empty());
    assert_eq!(
        app.world().resource::<WorldChanges>().0,
        3,
        "real chunk unloading must still invalidate dependent caches"
    );
}

#[test]
fn local_ground_condition_is_confined_to_one_cell_at_every_lod() {
    use super::{
        DetailLevel,
        builder::{RenderOptions, compose_section_with_options},
    };
    let mut templates = templates();
    templates.blocks.get_mut(&1).unwrap().details.clear();
    let face = BlockFace {
        edge_mask: 15,
        occupied_neighbor: false,
        minimum: [0; 3],
        width: 2,
        height: 1,
        u_axis: 0,
        v_axis: 2,
        side: 3,
        material: 1,
    };
    for detail_level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
        let options = RenderOptions {
            water_level: None,
            detail_level,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: [(
                [0, 0, 0],
                terra_world::environment::SurfaceCondition::new(0.0, 1.0).unwrap(),
            )]
            .into(),
        };
        let batches =
            compose_section_with_options(&[face], &templates, SectionPos::new(-1, 0, -1), &options)
                .unwrap();
        let batch = &batches[0];
        let vertices_per_cell = if detail_level == DetailLevel::Near {
            3
        } else {
            4
        };
        assert_eq!(batch.colors.len(), batch.positions.len());
        assert!(
            batch.colors[..vertices_per_cell]
                .iter()
                .all(|color| color[1] < 0.1)
        );
        assert!(
            batch.colors[vertices_per_cell..]
                .iter()
                .all(|color| *color == [1.0; 4])
        );
        assert!(
            batch
                .indices
                .iter()
                .all(|&index| (index as usize) < batch.positions.len())
        );
    }
}

#[test]
fn sparse_snow_cover_preserves_materials_and_neighbor_payloads_at_every_lod() {
    use super::{
        DetailLevel,
        builder::{RenderOptions, compose_section_with_options},
    };
    let mut templates = templates();
    templates.blocks.get_mut(&1).unwrap().details.clear();
    let mut snow = templates.blocks[&1].clone();
    snow.face_materials = [1; 6];
    for face in &mut snow.faces {
        for primitive in face {
            primitive.material = 1;
        }
    }
    templates.blocks.insert(5, snow);
    let face = BlockFace {
        edge_mask: 15,
        occupied_neighbor: false,
        minimum: [0; 3],
        width: 2,
        height: 1,
        u_axis: 0,
        v_axis: 2,
        side: 3,
        material: 1,
    };
    for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
        let options = RenderOptions {
            water_level: None,
            detail_level: level,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: [(
                [0; 3],
                terra_world::environment::SurfaceCondition {
                    snow: 1.0,
                    ..Default::default()
                },
            )]
            .into(),
        };
        let batches =
            compose_section_with_options(&[face], &templates, SectionPos::new(-1, 0, -1), &options)
                .unwrap();
        assert_eq!(batches.len(), 1);
        for batch in &batches {
            assert!(batch.rooted_wind.is_empty());
            assert_ne!(
                batch.material, 1,
                "Cosmetic snow must not switch to the snow material"
            );
            assert!(batch.surface_water.len() <= batch.positions.len());
            let mut surface = batch.surface_water.clone();
            surface.resize(batch.positions.len(), [0.0; 2]);
            assert!(batch.positions.iter().all(|p| p[0] >= 0.0 && p[0] <= 2.0));
            let vertices_per_cell = if level == DetailLevel::Near {
                templates.blocks[&1].faces[3]
                    .iter()
                    .map(|face| face.positions.len())
                    .sum()
            } else {
                4
            };
            assert_eq!(batch.positions.len(), vertices_per_cell * 2);
            for (positions, water) in batch
                .positions
                .chunks_exact(vertices_per_cell)
                .zip(surface.chunks_exact(vertices_per_cell))
            {
                let middle = positions.iter().map(|p| p[0]).sum::<f32>() / vertices_per_cell as f32;
                let expected = if middle < 1.0 {
                    [510.0, 1.0]
                } else {
                    [0.0, 0.0]
                };
                assert!(water.iter().all(|value| *value == expected));
            }
            assert!(
                batch
                    .indices
                    .iter()
                    .all(|&i| (i as usize) < batch.positions.len())
            );
        }
    }
}

#[test]
fn bare_natural_canopies_expose_only_snow_without_wetting_the_ground() {
    use super::builder::{RenderOptions, compose_section_with_options};
    let mut templates = templates();
    templates.blocks.get_mut(&1).unwrap().details.clear();
    let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig {
        biomes: false,
        trees: false,
        caves: false,
        water: false,
        ..Default::default()
    })
    .unwrap();
    let mut exposure = crate::weather::surfaces::exposure::Exposure {
        natural: None,
        roofs: [4; 256],
        snow_roofs: [4; 256],
        coast: [0.0; 256],
        coast_vertices: [0.0; 289],
        water: [f32::NEG_INFINITY; 256],
    };
    exposure.snow_roofs[0] = 0;
    let exposure = std::sync::Arc::new(exposure);
    let face = BlockFace {
        minimum: [0; 3],
        width: 2,
        height: 1,
        u_axis: 0,
        v_axis: 2,
        side: 3,
        material: 1,
        edge_mask: 15,
        occupied_neighbor: false,
    };
    for detail_level in [
        super::DetailLevel::Near,
        super::DetailLevel::Middle,
        super::DetailLevel::Far,
    ] {
        let options = RenderOptions {
            water_level: None,
            detail_level,
            terrain: Some(terrain.clone()),
            exposure: Some(exposure.clone()),
            tints: Default::default(),
            conditions: Default::default(),
        };
        let batches =
            compose_section_with_options(&[face], &templates, SectionPos::default(), &options)
                .unwrap();
        assert_eq!(batches.len(), 1);
        let batch = &batches[0];
        assert!(batch.rooted_wind.is_empty());
        let mut surface = batch.surface_water.clone();
        surface.resize(batch.positions.len(), [0.0; 2]);
        let vertices_per_cell = if detail_level == super::DetailLevel::Near {
            templates.blocks[&1].faces[3]
                .iter()
                .map(|face| face.positions.len())
                .sum()
        } else {
            4
        };
        assert_eq!(batch.positions.len(), vertices_per_cell * 2);
        for ((quad, water), colors) in batch
            .positions
            .chunks_exact(vertices_per_cell)
            .zip(surface.chunks_exact(vertices_per_cell))
            .zip(batch.colors.chunks_exact(vertices_per_cell))
        {
            let exposed = quad.iter().map(|p| p[0]).sum::<f32>() / (vertices_per_cell as f32) < 1.0;
            assert!(
                water
                    .iter()
                    .all(|value| *value == if exposed { [1024.0, 0.0] } else { [0.0; 2] })
            );
            assert!(colors.iter().all(|color| if exposed {
                color[3] > 0.0 && color[3] < 1.0
            } else {
                color[3] == 0.0
            }));
        }
    }
}

#[test]
fn dry_sand_and_snow_keep_imprint_uv1_under_roofs_at_every_terrain_lod() {
    use super::{
        DetailLevel,
        builder::{RenderOptions, compose_section_with_options},
    };
    let mut templates = templates();
    let mut cube = templates.blocks[&1].clone();
    cube.details.clear();
    for material in [3, 4, 5] {
        templates.blocks.insert(material, cube.clone());
    }
    let exposure = std::sync::Arc::new(crate::weather::surfaces::exposure::Exposure {
        natural: None,
        roofs: [64; 256],
        snow_roofs: [64; 256],
        coast: [0.0; 256],
        coast_vertices: [0.0; 289],
        water: [f32::NEG_INFINITY; 256],
    });
    for detail_level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
        for material in [3, 4, 5] {
            let face = BlockFace {
                minimum: [0; 3],
                width: 1,
                height: 1,
                u_axis: 0,
                v_axis: 2,
                side: 3,
                material,
                edge_mask: 15,
                occupied_neighbor: false,
            };
            let options = RenderOptions {
                water_level: None,
                detail_level,
                terrain: None,
                exposure: Some(exposure.clone()),
                tints: Default::default(),
                conditions: Default::default(),
            };
            let batches = compose_section_with_options(
                &[face],
                &templates,
                SectionPos::new(-1, 0, -1),
                &options,
            )
            .unwrap();
            assert!(!batches.is_empty());
            for batch in batches {
                if material == 3 {
                    assert!(batch.surface_water.is_empty());
                } else {
                    assert_eq!(batch.surface_water.len(), batch.positions.len());
                    assert!(
                        batch
                            .surface_water
                            .iter()
                            .all(|payload| *payload == [0.0; 2])
                    );
                    assert!(batch.rooted_wind.is_empty());
                }
            }
        }
    }
}

#[test]
fn wet_pbr_payload_is_sparse_and_never_wets_the_neighboring_dry_cell() {
    use super::{
        DetailLevel,
        builder::{RenderOptions, compose_section_with_options},
    };
    let mut templates = templates();
    templates.blocks.get_mut(&1).unwrap().details.clear();
    let face = BlockFace {
        edge_mask: 15,
        occupied_neighbor: false,
        minimum: [0; 3],
        width: 2,
        height: 1,
        u_axis: 0,
        v_axis: 2,
        side: 3,
        material: 1,
    };
    for detail_level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
        let mut options = RenderOptions {
            water_level: None,
            detail_level,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: [(
                [0; 3],
                terra_world::environment::SurfaceCondition::new(0.8, 0.0).unwrap(),
            )]
            .into(),
        };
        let batches =
            compose_section_with_options(&[face], &templates, SectionPos::new(-1, 0, -1), &options)
                .unwrap();
        let batch = &batches[0];
        assert!(!batch.surface_water.is_empty());
        for (index, position) in batch.positions.iter().enumerate() {
            let wet = batch.surface_water.get(index).map_or(0.0, |v| v[0]);
            if wet > 0.0 {
                assert!(position[0] <= 1.00001 && (wet - 0.8).abs() < 1e-6);
            }
        }
        options.conditions.clear();
        let dry =
            compose_section_with_options(&[face], &templates, SectionPos::new(-1, 0, -1), &options)
                .unwrap();
        assert!(dry.iter().all(|batch| batch.surface_water.is_empty()));
    }
}

#[test]
fn biome_vertex_colors_agree_at_shared_lod_vertices_and_negative_section_edges() {
    use super::{
        DetailLevel,
        builder::{RenderOptions, compose_section_with_options},
    };
    let mut templates = templates();
    templates.blocks.get_mut(&1).unwrap().details.clear();
    let terrain = terra_world::TerraGenerator::new(Default::default()).unwrap();
    let face = BlockFace {
        edge_mask: 15,
        occupied_neighbor: false,
        minimum: [0; 3],
        width: 16,
        height: 16,
        u_axis: 0,
        v_axis: 2,
        side: 3,
        material: 1,
    };
    let make = |detail_level, section| {
        compose_section_with_options(
            &[face],
            &templates,
            section,
            &RenderOptions {
                detail_level,
                water_level: None,
                terrain: Some(terrain.clone()),
                exposure: None,
                tints: Default::default(),
                conditions: Default::default(),
            },
        )
        .unwrap()
    };
    let near = make(DetailLevel::Near, SectionPos::new(-1, 0, -1));
    let far = make(DetailLevel::Far, SectionPos::new(-1, 0, -1));
    for (point, color) in far[0].positions.iter().zip(&far[0].colors) {
        for (_, other) in near[0]
            .positions
            .iter()
            .zip(&near[0].colors)
            .filter(|(p, _)| *p == point)
        {
            assert_eq!(other, color);
        }
    }
    let right = make(DetailLevel::Far, SectionPos::new(0, 0, -1));
    for (point, color) in far[0]
        .positions
        .iter()
        .zip(&far[0].colors)
        .filter(|(p, _)| p[0] == 16.0)
    {
        let index = right[0]
            .positions
            .iter()
            .position(|p| p[0] == 0.0 && p[2] == point[2])
            .unwrap();
        for (a, b) in color.iter().zip(right[0].colors[index]) {
            assert!((*a - b).abs() < 1.0e-6);
        }
    }
    assert_ne!(far[0].colors[0], [1.0; 4]);
}

#[test]
fn sparse_surface_edit_keeps_large_coarse_faces_greedy() {
    use super::{
        DetailLevel,
        builder::{RenderOptions, compose_section_with_options},
    };
    let mut templates = templates();
    templates.blocks.get_mut(&1).unwrap().details.clear();
    let face = BlockFace {
        edge_mask: 15,
        occupied_neighbor: false,
        minimum: [0; 3],
        width: 16,
        height: 16,
        u_axis: 0,
        v_axis: 2,
        side: 3,
        material: 1,
    };
    for detail_level in [DetailLevel::Middle, DetailLevel::Far] {
        let options = RenderOptions {
            water_level: None,
            detail_level,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: [(
                [7, 0, 7],
                terra_world::environment::SurfaceCondition::new(0.0, 1.0).unwrap(),
            )]
            .into(),
        };
        let batches =
            compose_section_with_options(&[face], &templates, SectionPos::default(), &options)
                .unwrap();
        assert_eq!(batches.len(), 1);
        let batch = &batches[0];
        assert!(
            batch.indices.len() <= 5 * 6,
            "one local edit must not expand 256 quads"
        );
        assert_eq!(batch.colors.len(), batch.positions.len());
        assert_eq!(
            batch.colors.iter().filter(|color| color[1] < 0.1).count(),
            4
        );
        assert!(
            batch
                .indices
                .iter()
                .all(|&index| (index as usize) < batch.positions.len())
        );
    }
}

#[test]
fn sand_and_snow_top_colors_are_seamless_without_splitting_greedy_faces() {
    use super::{
        DetailLevel,
        builder::{RenderOptions, compose_section_with_options},
    };
    let mut templates = templates();
    let mut block = templates.blocks[&1].clone();
    block.details.clear();
    for material in [4, 5] {
        templates.blocks.insert(material, block.clone());
    }
    let terrain = terra_world::TerraGenerator::new(Default::default()).unwrap();
    for material in [4, 5] {
        let face = BlockFace {
            edge_mask: 15,
            occupied_neighbor: false,
            minimum: [0; 3],
            width: 16,
            height: 16,
            u_axis: 0,
            v_axis: 2,
            side: 3,
            material,
        };
        for level in [DetailLevel::Middle, DetailLevel::Far] {
            let make = |x| {
                compose_section_with_options(
                    &[face],
                    &templates,
                    SectionPos::new(x, 0, -1),
                    &RenderOptions {
                        detail_level: level,
                        water_level: None,
                        terrain: Some(terrain.clone()),
                        exposure: None,
                        tints: Default::default(),
                        conditions: Default::default(),
                    },
                )
                .unwrap()
            };
            let left = make(-1);
            let right = make(0);
            assert_eq!(left[0].positions.len(), 4);
            assert_eq!(left[0].indices.len(), 6);
            for (point, color) in left[0]
                .positions
                .iter()
                .zip(&left[0].colors)
                .filter(|(p, _)| p[0] == 16.0)
            {
                let index = right[0]
                    .positions
                    .iter()
                    .position(|p| p[0] == 0.0 && p[2] == point[2])
                    .unwrap();
                for (a, b) in color.iter().zip(right[0].colors[index]) {
                    assert!((a - b).abs() < 1e-6);
                }
            }
        }
    }
}

#[test]
fn meadow_spirit_and_blood_turf_share_climate_tints_on_top_and_side_in_every_lod() {
    use super::builder::{RenderOptions, compose_section_with_options};
    let mut templates = templates();
    let mut turf = templates.blocks[&1].clone();
    turf.details.clear();
    turf.faces = crate::blocks::cube_faces(0.0, [0; 6], None);
    templates.blocks.insert(1, turf.clone());
    templates.cube_lods.insert(1, turf.clone());
    for id in [306, 400] {
        templates.blocks.insert(id, turf.clone());
    }
    let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig {
        caves: false,
        cave_structure_revision: 0,
        structures_revision: 0,
        rivers: false,
        ..Default::default()
    })
    .unwrap();
    for section in [SectionPos::new(-1, 3, -1), SectionPos::new(0, 3, -1)] {
        let expected = crate::appearance::GroundTint::new(&terrain, section.origin().unwrap());
        for side in [1, 3, 5] {
            for level in [
                super::DetailLevel::Near,
                super::DetailLevel::Middle,
                super::DetailLevel::Far,
            ] {
                for material in [1, 306, 400] {
                    let face = BlockFace {
                        minimum: [0; 3],
                        width: 4,
                        height: 4,
                        u_axis: if side == 1 { 2 } else { 0 },
                        v_axis: if side == 3 { 2 } else { 1 },
                        side,
                        material,
                        edge_mask: 15,
                        occupied_neighbor: false,
                    };
                    let batches = compose_section_with_options(
                        &[face],
                        &templates,
                        section,
                        &RenderOptions {
                            water_level: None,
                            detail_level: level,
                            terrain: Some(terrain.clone()),
                            exposure: None,
                            tints: Default::default(),
                            conditions: Default::default(),
                        },
                    )
                    .unwrap();
                    assert!(!batches.is_empty());
                    for batch in batches {
                        assert_eq!(batch.surface_water.len(), batch.positions.len());
                        for (position, color) in batch.positions.into_iter().zip(batch.colors) {
                            let tint = expected.authored_grass_at(position);
                            assert!(tint.into_iter().any(|v| (v - 1.0).abs() > 0.001));
                            for axis in 0..3 {
                                assert!(
                                    (color[axis] - tint[axis]).abs() < 1e-6,
                                    "material {material}, side {side}, lod {level:?}: {position:?}: {color:?} vs {tint:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
