use super::wind::PropMaterial;
use super::*;
use crate::{blocks::PropMaterials, meshing::DetailLevel};
use bevy::prelude::*;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use terra_voxel::{BlockSection, SectionPos};
use terra_world::props::PropKind;
use terra_world::{
    edits::EditJournal,
    props::{PropId, WorldProp},
    streaming::{ChunkFocus, ChunkStore, LoadedSection, WorldTerrain},
};

#[test]
fn micro_geometry_has_valid_indices_outward_normals_and_bounded_cost() {
    for kind in [
        PropKind::Grass,
        PropKind::Flower,
        PropKind::Bush,
        PropKind::Fern,
        PropKind::Mushroom,
        PropKind::SmallPlant,
    ] {
        for variant in 0..=u8::MAX {
            for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
                let batches =
                    shapes::template(kind, variant, level, &PropMaterials::default()).unwrap();
                let vertices: usize = batches.iter().map(|b| b.positions.len()).sum();
                assert!(
                    vertices > 0,
                    "{kind:?} must retain a silhouette even in Far LOD"
                );
                let budget = match kind {
                    PropKind::Grass => 1168,
                    PropKind::Mushroom => 1128,
                    PropKind::Bush => 2112,
                    PropKind::Flower => 1836,
                    _ => 536,
                };
                assert!(vertices <= budget, "{kind:?}: {vertices}");
                for batch in batches {
                    assert_eq!(batch.positions.len(), batch.normals.len());
                    assert_eq!(batch.positions.len(), batch.uvs.len());
                    assert!(batch.colors.is_empty() || batch.colors.len() == batch.positions.len());
                    assert!(batch.positions.iter().flatten().all(|p| p.is_finite()));
                    assert_eq!(batch.indices.len() % 3, 0);
                    assert!(
                        batch
                            .indices
                            .iter()
                            .all(|index| (*index as usize) < batch.positions.len())
                    );
                    assert!(batch.normals.iter().all(|normal| {
                        (Vec3::from_array(*normal).length_squared() - 1.0).abs() < 1e-5
                    }));
                    for tri in batch.indices.chunks_exact(3) {
                        let [a, b, c] = std::array::from_fn(|i| {
                            Vec3::from_array(batch.positions[tri[i] as usize])
                        });
                        let n = Vec3::from_array(batch.normals[tri[0] as usize]);
                        assert!((b - a).cross(c - a).dot(n) > 0.0);
                    }
                }
            }
        }
    }
}

#[test]
fn botanical_detail_is_deterministic_with_bounded_lod_cost() {
    let palette = PropMaterials {
        foliage: [0, 1, 2],
        stem: 3,
        flowers: [4, 5, 6, 7, 8, 9, 16, 17],
        mushroom: 10,
        stone: 11,
        dry_grass: [12, 13, 14],
        twig: 15,
        crop: 2,
    };
    for variant in 0..=u8::MAX {
        for (kind, budgets) in [
            (
                PropKind::Grass,
                [
                    832 + usize::from(variant < 128 && variant % 8 % 3 == 2) * 184,
                    160,
                    32,
                ],
            ),
            (
                PropKind::Flower,
                [
                    match variant % 8 {
                        0 | 4 => 1260,
                        5 => 1836,
                        6 => 828,
                        _ => 1692,
                    },
                    144,
                    48,
                ],
            ),
            (
                PropKind::Bush,
                if variant >= 128 {
                    [336, 216, 168]
                } else {
                    [
                        match variant % 8 {
                            3 | 4 => 1968,
                            5 | 6 => 2112,
                            _ => 1680,
                        },
                        168,
                        96,
                    ]
                },
            ),
        ] {
            for (level, budget) in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far]
                .into_iter()
                .zip(budgets)
            {
                let first = shapes::template(kind, variant, level, &palette).unwrap();
                let second = shapes::template(kind, variant, level, &palette).unwrap();
                assert_eq!(
                    first
                        .iter()
                        .map(|batch| batch.positions.len())
                        .sum::<usize>(),
                    budget
                );
                assert_eq!(first.len(), second.len());
                for (first, second) in first.iter().zip(&second) {
                    assert_eq!(first.material, second.material);
                    assert_eq!(first.positions, second.positions);
                    assert_eq!(first.normals, second.normals);
                    assert_eq!(first.indices, second.indices);
                    assert_eq!(first.uvs, second.uvs);
                    assert_eq!(first.colors, second.colors);
                }
                if kind == PropKind::Flower {
                    assert!(first.iter().any(|batch| {
                        batch.material == palette.flowers[usize::from(variant) % 8]
                    }));
                }
            }
        }
    }
}

#[test]
fn flower_variants_have_cupped_radial_and_layered_petal_silhouettes() {
    let palette = PropMaterials {
        foliage: [0, 1, 2],
        stem: 3,
        flowers: [4, 5, 6, 7, 8, 9, 16, 17],
        mushroom: 10,
        stone: 11,
        ..default()
    };
    let heads: Vec<_> = (0..3)
        .map(|variant| {
            let geometry =
                shapes::template(PropKind::Flower, variant, DetailLevel::Near, &palette).unwrap();
            let height = geometry
                .iter()
                .flat_map(|batch| &batch.positions)
                .map(|point| point[1])
                .fold(0.0, f32::max);
            assert!((height - 1.0).abs() < 1e-6);
            let head = geometry
                .into_iter()
                .find(|batch| batch.material == palette.flowers[usize::from(variant)])
                .unwrap();
            (head, height)
        })
        .collect();
    assert_eq!(heads[0].0.positions.len(), 9 * 24 * 3);
    assert_eq!(heads[1].0.positions.len(), 16 * 24 * 3);
    assert_eq!(heads[2].0.positions.len(), 15 * 24 * 3);
    let first_petal_height = |batch: &crate::meshing::builder::MeshBatch| {
        let points = &batch.positions[..24];
        let low = points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let high = points
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max);
        high - low
    };
    assert!(first_petal_height(&heads[0].0) > first_petal_height(&heads[1].0) * 1.3);
    assert!(first_petal_height(&heads[2].0) < heads[2].1 * 0.12);
}

fn prop(id: u64, section: SectionPos) -> WorldProp {
    let origin = section.origin().unwrap();
    WorldProp {
        id: PropId(id),
        kind: PropKind::Bush,
        position: [
            origin[0] as f64 + 1.5 + id as f64 % 5.0,
            origin[1] as f64 + 1.0,
            origin[2] as f64 + 3.5,
        ],
        rotation: 0.7,
        scale: 0.8,
        variant: 0,
        section,
        removed: false,
    }
}

fn section(position: SectionPos, props: Vec<WorldProp>) -> LoadedSection {
    let mut data = BlockSection::empty(position).unwrap();
    for z in 0..16 {
        for x in 0..16 {
            data.set_block([x, 0, z], 3).unwrap();
        }
    }
    for prop in &props {
        let (owner, local) =
            terra_voxel::global_to_section(terra_world::props::placement::support(prop));
        if owner == position {
            data.set_block(local, 3).unwrap();
        }
    }
    LoadedSection {
        data,
        mesh: Some(terra_voxel::meshing::BlockMesh {
            positions: vec![[1.0, 2.0, 3.0]],
            ..default()
        }),
        mesh_epoch: 0,
        water_epoch: 0,
        water: None,
        props,
    }
}

fn app() -> App {
    use crate::blocks::{BlockAssets, TemplateSet};
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<PropMaterial>>()
        .init_resource::<BlockAssets>()
        .init_resource::<EditJournal>()
        .init_resource::<ChunkStore>()
        .init_resource::<terra_core::GameConfig>()
        .init_resource::<crate::weather::storage::WeatherPayload>()
        .add_systems(PostUpdate, crate::weather::storage::update)
        .insert_resource(WorldTerrain(
            terra_world::TerraGenerator::new(terra_core::WorldConfig {
                biomes: false,
                ..default()
            })
            .unwrap(),
        ))
        .insert_resource(ChunkFocus([8.0, 8.0, 8.0]));
    let colors = [
        Color::srgb(0.16, 0.32, 0.075),
        Color::srgb(0.28, 0.46, 0.12),
        Color::srgb(0.40, 0.57, 0.20),
        Color::srgb(0.18, 0.29, 0.075),
        Color::srgb(0.69, 0.33, 0.51),
        Color::srgb(0.91, 0.73, 0.22),
        Color::srgb(0.83, 0.88, 0.80),
        Color::srgb(0.48, 0.32, 0.67),
        Color::srgb(0.60, 0.16, 0.09),
        Color::srgb(0.46, 0.49, 0.43),
    ];
    let handles = colors.map(|color| {
        app.world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: color,
                ..default()
            })
    });
    let mut assets = app.world_mut().resource_mut::<BlockAssets>();
    assets.materials.extend(handles);
    assets.templates = Some(Arc::new(TemplateSet {
        prop_materials: PropMaterials {
            foliage: [0, 1, 2],
            stem: 3,
            flowers: [4, 5, 6, 7, 4, 7, 5, 6],
            mushroom: 8,
            stone: 9,
            dry_grass: [4, 5, 6],
            twig: 8,
            crop: 2,
        },
        ..default()
    }));
    jobs::install(&mut app);
    crate::transitions::install(&mut app);
    app
}

fn settle(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        let status = app.world().resource::<PropRenderStatus>();
        assert!(status.error.is_none(), "{:?}", status.error);
        if status.ready {
            return;
        }
        assert!(Instant::now() < deadline, "prop render jobs timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn dense_prop_sections_keep_bounded_shared_batches_and_release_them_on_unload() {
    let mut app = app();
    let owner = SectionPos::default();
    let props = (1..=64)
        .map(|id| WorldProp {
            position: [
                1.5 + ((id - 1) % 8) as f64 * 1.5,
                1.0,
                1.5 + ((id - 1) / 8) as f64 * 1.5,
            ],
            ..prop(id, owner)
        })
        .collect();
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(owner, section(owner, props));
    app.init_resource::<crate::MeshUploadStatus>()
        .add_systems(First, crate::MeshUploadStatus::reset);
    settle(&mut app);
    let rendered = jobs::testing::snapshot(app.world(), owner).unwrap();
    assert_eq!(rendered.count, 64);
    assert!(rendered.meshes.len() > 1);
    let bush_batches = shapes::template(
        PropKind::Bush,
        0,
        DetailLevel::Near,
        &PropMaterials::default(),
    )
    .unwrap();
    let expected_vertices = bush_batches
        .iter()
        .map(|batch| batch.positions.len())
        .sum::<usize>()
        * 64;
    let expected_indices = bush_batches
        .iter()
        .map(|batch| batch.indices.len())
        .sum::<usize>()
        * 64;
    let mut vertices = 0;
    let mut indices = 0;
    for id in &rendered.meshes {
        let mesh = app.world().resource::<Assets<Mesh>>().get(*id).unwrap();
        assert!(mesh.count_vertices() <= usize::from(u16::MAX) + 1);
        vertices += mesh.count_vertices();
        indices += mesh.indices().unwrap().len();
    }
    assert_eq!(
        vertices, expected_vertices,
        "all 64 bushes retain their full geometry"
    );
    assert_eq!(indices, expected_indices);
    assert!(vertices > crate::upload_budget::PROP_VERTICES);
    assert!(
        app.world()
            .resource::<crate::MeshUploadStatus>()
            .peak_admitted_vertices
            <= crate::upload_budget::PROP_VERTICES
    );
    let mut query = app
        .world_mut()
        .query::<(&Mesh3d, &MeshMaterial3d<PropMaterial>)>();
    let materials = query
        .iter(app.world())
        .filter(|(mesh, _)| rendered.meshes.contains(&mesh.0.id()))
        .map(|(_, material)| material.0.id())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(materials.len(), 1, "pieces still share one material");
    app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
    settle(&mut app);
    assert!(app.world().resource::<Assets<Mesh>>().is_empty());
}

#[test]
fn prop_selection_retries_only_finished_roots_after_entry() {
    let mut app = app();
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_secs_f32(1.0 / 30.0),
    ));
    {
        let mut config = app.world_mut().resource_mut::<terra_core::GameConfig>();
        config.client.chunk_animation = terra_core::ChunkAnimation::Fade;
        config.client.max_mesh_jobs = 2;
    }
    let owner = SectionPos::default();
    let stable = SectionPos::new(100, 0, 0);
    for position in [owner, stable] {
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(position, section(position, vec![prop(1, position)]));
    }
    let root_for = |world: &mut World, position| {
        let mesh = *jobs::testing::snapshot(world, position)?.meshes.first()?;
        world
            .query::<(&Mesh3d, &ChildOf)>()
            .iter(world)
            .find_map(|(handle, parent)| (handle.0.id() == mesh).then_some(parent.parent()))
    };
    let vertices = |world: &World, position| {
        jobs::testing::snapshot(world, position)
            .unwrap()
            .meshes
            .iter()
            .map(|id| {
                world
                    .resource::<Assets<Mesh>>()
                    .get(*id)
                    .unwrap()
                    .count_vertices()
            })
            .sum::<usize>()
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    let (owner_root, stable_root) = loop {
        app.update();
        let roots = (
            root_for(app.world_mut(), owner),
            root_for(app.world_mut(), stable),
        );
        if let (Some(owner_root), Some(stable_root)) = roots
            && [owner_root, stable_root].into_iter().all(|entity| {
                app.world()
                    .get::<crate::transitions::ChunkFade>(entity)
                    .is_some()
            })
            && jobs::testing::pending(app.world()) == 0
        {
            break (owner_root, stable_root);
        }
        assert!(Instant::now() < deadline, "prop entry fades did not start");
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(vertices(app.world(), owner), 1680);
    assert_eq!(vertices(app.world(), stable), 96);
    let owner_before = jobs::testing::snapshot(app.world(), owner).unwrap();
    let stable_before = jobs::testing::snapshot(app.world(), stable).unwrap();
    let completed = app.world().resource::<PropRenderStatus>().completed_jobs;
    app.world_mut().resource_mut::<ChunkFocus>().0 = [312.0, 8.0, 8.0];
    app.update();
    assert_eq!(jobs::testing::pending(app.world()), 0);
    assert_eq!(
        jobs::testing::snapshot(app.world(), owner).unwrap(),
        owner_before
    );
    assert!(
        app.world()
            .get::<crate::transitions::ChunkFade>(owner_root)
            .is_some()
    );
    let unrelated = app
        .world_mut()
        .spawn(crate::transitions::EnteringChunk)
        .id();
    let retired = jobs::testing::spawn_retired_root(app.world_mut(), owner);
    for entity in [unrelated, retired] {
        app.world_mut()
            .entity_mut(entity)
            .remove::<crate::transitions::EnteringChunk>();
    }
    app.update();
    assert_eq!(jobs::testing::pending(app.world()), 0);
    assert_eq!(
        app.world().resource::<PropRenderStatus>().completed_jobs,
        completed
    );
    assert_eq!(
        jobs::testing::snapshot(app.world(), owner).unwrap(),
        owner_before
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
        if vertices(app.world(), owner) == 96 && jobs::testing::pending(app.world()) == 0 {
            assert_ne!(root_for(app.world_mut(), owner).unwrap(), owner_root);
            assert_eq!(root_for(app.world_mut(), stable).unwrap(), stable_root);
            assert_eq!(
                jobs::testing::snapshot(app.world(), stable).unwrap(),
                stable_before
            );
            assert_eq!(
                app.world().resource::<PropRenderStatus>().completed_jobs,
                completed + 1
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "finished prop entry lost its stationary LOD retry"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn far_props_keep_every_identity_with_packed_aligned_gpu_attributes() {
    let mut app = app();
    let owner = SectionPos::default();
    app.world_mut().resource_mut::<ChunkFocus>().0 = [208.0, 8.0, 8.0];
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        owner,
        section(
            owner,
            vec![WorldProp {
                kind: PropKind::Grass,
                ..prop(1, owner)
            }],
        ),
    );
    settle(&mut app);
    let snapshot = jobs::testing::snapshot(app.world(), owner).unwrap();
    assert_eq!(snapshot.count, 1);
    let mesh = app
        .world()
        .resource::<Assets<Mesh>>()
        .get(snapshot.meshes[0])
        .unwrap();
    assert_eq!(mesh.count_vertices(), 32);
    assert!(matches!(
        mesh.attribute(Mesh::ATTRIBUTE_COLOR),
        Some(bevy::mesh::VertexAttributeValues::Unorm8x4(_))
    ));
    assert!(matches!(
        mesh.attribute(Mesh::ATTRIBUTE_NORMAL),
        Some(bevy::mesh::VertexAttributeValues::Snorm8x4(_))
    ));
    let mut store = bevy::mesh::MeshVertexBufferLayouts::default();
    let buffer = mesh.get_mesh_vertex_buffer_layout(&mut store);
    let layout = buffer
        .0
        .get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(5),
        ])
        .unwrap();
    assert_eq!(layout.array_stride, 28);
    assert_eq!(layout.array_stride % 4, 0);
}

#[test]
fn grass_growth_uses_small_rooted_gpu_batch_without_per_frame_mesh_rebuilds_or_reload_pop() {
    use bevy::mesh::VertexAttributeValues;
    let mut app = app();
    app.add_systems(Update, wind::update_growth_clock);
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_millis(100),
    ));
    let owner = SectionPos::default();
    let saved = WorldProp {
        kind: PropKind::Grass,
        variant: 64,
        scale: 0.12,
        ..prop(101, owner)
    };
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(owner, section(owner, vec![prop(1, owner), saved]));
    settle(&mut app);
    assert_eq!(app.world().resource::<PropGrowthStatus>().transitions, 0);
    let snapshot = jobs::testing::snapshot(app.world(), owner).unwrap();
    assert_eq!(
        snapshot.meshes.len(),
        2,
        "regular geometry and the small growth-only batch"
    );
    let timing = |app: &App| {
        let snapshot = jobs::testing::snapshot(app.world(), owner).unwrap();
        snapshot
            .meshes
            .iter()
            .filter_map(|id| app.world().resource::<Assets<Mesh>>().get(*id))
            .filter(|mesh| mesh.contains_attribute(wind::ATTRIBUTE_GROWTH_ANCHOR))
            .flat_map(|mesh| match mesh.attribute(Mesh::ATTRIBUTE_UV_1) {
                Some(VertexAttributeValues::Float32x2(values)) => values.clone(),
                _ => Vec::new(),
            })
            .collect::<Vec<_>>()
    };
    assert!(timing(&app).iter().all(|transition| transition[1] == 1.0));
    let fresh = WorldProp {
        id: PropId(202),
        ..saved
    };
    app.world_mut()
        .resource_mut::<EditJournal>()
        .place_prop(fresh)
        .unwrap();
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&owner)
        .unwrap()
        .props
        .push(fresh);
    settle(&mut app);
    assert_eq!(app.world().resource::<PropGrowthStatus>().transitions, 1);
    let timing_fresh = timing(&app);
    assert!(timing_fresh.iter().any(|transition| transition[1] == 0.0));
    assert!(
        timing_fresh.iter().all(|transition| transition[0] >= 0.0),
        "transition clock starts at upload"
    );
    let completed = app.world().resource::<PropRenderStatus>().completed_jobs;
    let meshes = jobs::testing::snapshot(app.world(), owner).unwrap().meshes;
    for _ in 0..30 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<PropRenderStatus>().completed_jobs,
        completed
    );
    assert_eq!(
        jobs::testing::snapshot(app.world(), owner).unwrap().meshes,
        meshes
    );
    let clock = app.world().resource::<PropGrowthStatus>();
    assert!((clock.shader_time - clock.active_until).abs() < 1e-6);
    let mut delta = app.world().resource::<EditJournal>().section_delta(owner);
    let fresh = delta
        .placed_props
        .iter_mut()
        .find(|prop| prop.id == fresh.id)
        .unwrap();
    fresh.variant = 65;
    fresh.scale = 0.25;
    delta.prop_epoch += 1;
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            world
                .resource_mut::<ChunkStore>()
                .apply_confirmed_delta(&mut journal, delta)
                .unwrap();
        });
    settle(&mut app);
    assert_eq!(app.world().resource::<PropGrowthStatus>().transitions, 2);
    assert!(
        timing(&app)
            .iter()
            .any(|transition| (transition[1] - 0.48).abs() < 0.001)
    );
}

fn remove_prop(app: &mut App, selected: WorldProp) {
    app.world_mut()
        .resource_mut::<EditJournal>()
        .remove_prop(selected.section, selected.id)
        .unwrap();
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&selected.section)
        .unwrap()
        .props
        .iter_mut()
        .find(|prop| prop.id == selected.id)
        .unwrap()
        .removed = true;
}

#[test]
fn deleting_one_prop_rebuilds_only_its_batch_and_preserves_terrain_meshes() {
    let mut app = app();
    let owner = SectionPos::default();
    let neighbor = SectionPos::new(1, 0, 0);
    let selected = prop(1, owner);
    app.world_mut().resource_mut::<ChunkStore>().loaded.extend([
        (owner, section(owner, vec![selected, prop(2, owner)])),
        (neighbor, section(neighbor, vec![prop(3, neighbor)])),
    ]);
    settle(&mut app);
    let before = jobs::testing::snapshot(app.world(), owner).unwrap();
    let next_before = jobs::testing::snapshot(app.world(), neighbor).unwrap();
    let mesh_count = app.world().resource::<Assets<Mesh>>().len();
    assert_eq!(mesh_count, 2, "one shared-color mesh per occupied section");
    assert_eq!(app.world().resource::<PropRenderStatus>().visible_props, 3);
    app.world_mut()
        .resource_mut::<terra_core::GameConfig>()
        .client
        .vegetation_density = 0.0;
    settle(&mut app);
    assert_eq!(app.world().resource::<PropRenderStatus>().visible_props, 3);
    assert_eq!(jobs::testing::snapshot(app.world(), owner).unwrap(), before);
    remove_prop(&mut app, selected);
    settle(&mut app);
    let after = jobs::testing::snapshot(app.world(), owner).unwrap();
    assert_eq!(after.prop_epoch, 1);
    assert_eq!(after.terrain_epoch, before.terrain_epoch);
    assert_eq!(after.count, 1);
    assert_ne!(after.meshes, before.meshes);
    assert_eq!(
        jobs::testing::snapshot(app.world(), neighbor).unwrap(),
        next_before
    );
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), mesh_count);
    assert!(
        before
            .meshes
            .iter()
            .all(|id| app.world().resource::<Assets<Mesh>>().get(*id).is_none())
    );
    let chunks = app.world().resource::<ChunkStore>();
    assert_eq!(
        chunks.loaded[&owner].mesh.as_ref().unwrap().positions,
        vec![[1.0, 2.0, 3.0]]
    );
    assert!(!chunks.loaded[&owner].props[1].removed);
    assert_eq!(app.world().resource::<EditJournal>().mesh_epoch(owner), 0);
    assert_eq!(app.world().resource::<PropRenderStatus>().visible_props, 2);
}

#[test]
fn all_prop_colors_share_one_mesh_and_material_with_linear_vertex_colors() {
    use crate::blocks::BlockAssets;
    use bevy::mesh::VertexAttributeValues;
    let mut app = app();
    let owner = SectionPos::default();
    let neighbor = SectionPos::new(1, 0, 0);
    let kinds = [
        PropKind::Grass,
        PropKind::Flower,
        PropKind::Bush,
        PropKind::Fern,
        PropKind::Mushroom,
        PropKind::SmallPlant,
        PropKind::SmallPlant,
    ];
    for section_pos in [owner, neighbor] {
        let props = kinds
            .into_iter()
            .enumerate()
            .map(|(i, kind)| WorldProp {
                kind,
                variant: if i == 6 { 128 } else { 0 },
                ..prop(i as u64, section_pos)
            })
            .collect();
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(section_pos, section(section_pos, props));
    }
    settle(&mut app);
    assert_eq!(app.world().resource::<PropRenderStatus>().batches, 2);
    assert_eq!(app.world().resource::<PropRenderStatus>().visible_props, 14);
    let mesh_id = jobs::testing::snapshot(app.world(), owner).unwrap().meshes[0];
    let mesh = app.world().resource::<Assets<Mesh>>().get(mesh_id).unwrap();
    let Some(VertexAttributeValues::Unorm8x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
    else {
        panic!("missing vertex colors");
    };
    assert_eq!(colors.len(), mesh.count_vertices());
    let materials = app.world().resource::<Assets<StandardMaterial>>();
    let source_colors: Vec<_> = app
        .world()
        .resource::<BlockAssets>()
        .materials
        .iter()
        .map(|handle| {
            let color = materials.get(handle).unwrap().base_color.to_linear();
            [color.red, color.green, color.blue, color.alpha]
        })
        .collect();
    let biome = crate::appearance::GrassTint::new(
        &app.world().resource::<WorldTerrain>().0,
        owner.origin().unwrap(),
    )
    .relative_at([0.0; 3]);
    let tinted_colors: Vec<_> = source_colors
        .iter()
        .enumerate()
        .map(|(index, color)| {
            let mut color = *color;
            if index < 4 {
                for channel in 0..3 {
                    color[channel] *= biome[channel];
                }
            }
            color
        })
        .collect();
    let base = [0.77, 0.87, 0.70];
    let tip = [1.22, 1.17, 0.81];
    let palette = &app
        .world()
        .resource::<BlockAssets>()
        .templates
        .as_ref()
        .unwrap()
        .prop_materials;
    let mut expected_colors = Vec::new();
    for (i, kind) in kinds.into_iter().enumerate() {
        let rank = terra_world::props::seasonal::flower_rank(&WorldProp {
            kind,
            ..prop(i as u64, owner)
        });
        for part in shapes::template(
            kind,
            if i == 6 { 128 } else { 0 },
            DetailLevel::Near,
            palette,
        )
        .unwrap()
        {
            let source = source_colors[part.material];
            let green = kind != PropKind::Mushroom
                && (palette.foliage.contains(&part.material) || part.material == palette.stem);
            for vertex in 0..part.positions.len() {
                let shade = part.colors.get(vertex).copied().unwrap_or([1.0; 4]);
                expected_colors.push(crate::foliage::packing::color([
                    source[0] * shade[0] * if green { biome[0] } else { 1.0 },
                    source[1] * shade[1] * if green { biome[1] } else { 1.0 },
                    source[2] * shade[2] * if green { biome[2] } else { 1.0 },
                    rank,
                ]));
            }
        }
    }
    assert!(colors.iter().all(|color| expected_colors.contains(color)));
    for shade in [base, tip] {
        let source = source_colors[0];
        let expected = crate::foliage::packing::color([
            source[0] * shade[0] * biome[0],
            source[1] * shade[1] * biome[1],
            source[2] * shade[2] * biome[2],
            source[3],
        ]);
        assert!(
            colors.contains(&expected),
            "dark roots and warm square tips must survive linear batching"
        );
    }
    assert!(colors.contains(&crate::foliage::packing::color(tinted_colors[0])));
    for (kind, material) in [
        (PropKind::Flower, palette.flowers[0]),
        (PropKind::Mushroom, palette.flowers[5]),
    ] {
        let part = shapes::template(kind, 0, DetailLevel::Near, palette)
            .unwrap()
            .into_iter()
            .find(|part| part.material == material)
            .unwrap();
        let shade = part.colors.first().copied().unwrap_or([1.0; 4]);
        let mut expected = std::array::from_fn(|axis| source_colors[material][axis] * shade[axis]);
        let index = kinds
            .iter()
            .position(|candidate| *candidate == kind)
            .unwrap();
        expected[3] = terra_world::props::seasonal::flower_rank(&WorldProp {
            kind,
            ..prop(index as u64, owner)
        });
        assert!(
            colors.contains(&crate::foliage::packing::color(expected)),
            "{kind:?} keeps its authored material and local surface shading"
        );
    }
    assert!(colors.contains(&crate::foliage::packing::color(
        source_colors[palette.stone]
    )));
    assert_ne!(
        source_colors[0],
        [0.16, 0.32, 0.075, 1.0],
        "sRGB must be converted to linear before upload"
    );
    let handles: Vec<_> = app
        .world_mut()
        .query::<&MeshMaterial3d<PropMaterial>>()
        .iter(app.world())
        .map(|material| material.0.clone())
        .collect();
    assert_eq!(handles.len(), 2);
    assert_eq!(handles[0], handles[1]);
    let white = app
        .world()
        .resource::<Assets<PropMaterial>>()
        .get(&handles[0])
        .unwrap();
    assert_eq!(white.base.base_color.to_linear(), Color::WHITE.to_linear());
    assert!(white.base.base_color_texture.is_none());
}

#[test]
fn eviction_releases_prop_meshes_and_reloaded_removals_are_not_rendered() {
    let mut app = app();
    let owner = SectionPos::new(-1, 0, -1);
    let selected = prop(1, owner);
    let original = vec![selected, prop(2, owner)];
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(owner, section(owner, original.clone()));
    settle(&mut app);
    remove_prop(&mut app, selected);
    settle(&mut app);
    app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
    settle(&mut app);
    assert!(app.world().resource::<Assets<Mesh>>().is_empty());
    assert_eq!(app.world().resource::<PropRenderStatus>().visible_props, 0);
    let mut reloaded = original;
    app.world()
        .resource::<EditJournal>()
        .apply_prop_removals(owner, &mut reloaded);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(owner, section(owner, reloaded));
    settle(&mut app);
    assert_eq!(
        jobs::testing::snapshot(app.world(), owner).unwrap().count,
        1
    );
    assert_eq!(app.world().resource::<PropRenderStatus>().visible_props, 1);
}

#[test]
fn stale_prop_jobs_never_replace_the_visible_batch_after_removal() {
    let mut app = app();
    let owner = SectionPos::default();
    let selected = prop(1, owner);
    let props = vec![selected, prop(2, owner)];
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(owner, section(owner, props.clone()));
    settle(&mut app);
    let original = jobs::testing::snapshot(app.world(), owner).unwrap();
    jobs::testing::enqueue(app.world_mut(), owner, 0, 0, props);
    remove_prop(&mut app, selected);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        let visible = jobs::testing::snapshot(app.world(), owner).unwrap();
        if visible.prop_epoch == 1 {
            assert_eq!(visible.count, 1);
            break;
        }
        assert_eq!(visible, original, "stale batch became visible");
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn stale_prop_job_cannot_erase_a_newly_placed_prop_and_reload_replays_it_once() {
    let mut app = app();
    let owner = SectionPos::default();
    let original_props = vec![prop(1, owner)];
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(owner, section(owner, original_props.clone()));
    settle(&mut app);
    let original = jobs::testing::snapshot(app.world(), owner).unwrap();
    jobs::testing::enqueue(app.world_mut(), owner, 0, 0, original_props.clone());
    let placed = prop(9877, owner);
    assert!(
        app.world_mut()
            .resource_mut::<EditJournal>()
            .place_prop(placed)
            .unwrap()
    );
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&owner)
        .unwrap()
        .props
        .push(placed);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        let visible = jobs::testing::snapshot(app.world(), owner).unwrap();
        if visible.prop_epoch == 1 {
            assert_eq!(visible.count, 2);
            break;
        }
        assert_eq!(
            visible, original,
            "stale pre-placement batch became visible"
        );
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
    settle(&mut app);
    let mut reloaded = original_props;
    let journal = app.world().resource::<EditJournal>();
    journal.apply_prop_removals(owner, &mut reloaded);
    journal.apply_prop_removals(owner, &mut reloaded);
    assert_eq!(
        reloaded.iter().filter(|prop| prop.id == placed.id).count(),
        1
    );
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(owner, section(owner, reloaded));
    settle(&mut app);
    assert_eq!(
        jobs::testing::snapshot(app.world(), owner).unwrap().count,
        2
    );
}

#[test]
fn stale_terrain_jobs_cannot_upload_after_support_edits() {
    let mut app = app();
    let owner = SectionPos::default();
    let props = vec![prop(1, owner)];
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(owner, section(owner, props.clone()));
    settle(&mut app);
    let original = jobs::testing::snapshot(app.world(), owner).unwrap();
    jobs::testing::enqueue(app.world_mut(), owner, 0, 0, props);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record([1; 3], 3)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        assert_eq!(
            jobs::testing::snapshot(app.world(), owner).unwrap(),
            original
        );
        if jobs::testing::pending(app.world()) == 0 {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn first_prop_geometry_after_an_empty_batch_enters_the_animation_queue() {
    for mode in [
        terra_core::ChunkAnimation::None,
        terra_core::ChunkAnimation::Fade,
    ] {
        let mut app = app();
        app.world_mut()
            .resource_mut::<terra_core::GameConfig>()
            .client
            .chunk_animation = mode;
        app.world_mut().spawn((
            terra_player::TerraCamera,
            bevy::camera::primitives::Frustum::default(),
            GlobalTransform::default(),
        ));
        let owner = SectionPos::new(-1, 0, -1);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(owner, section(owner, Vec::new()));
        settle(&mut app);
        assert!(
            jobs::testing::snapshot(app.world(), owner)
                .is_none_or(|empty| empty.count == 0 && empty.meshes.is_empty()),
            "Untouched empty sections do not need a render-cache entry"
        );

        let placed = prop(9877, owner);
        app.world_mut()
            .resource_mut::<EditJournal>()
            .place_prop(placed)
            .unwrap();
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&owner)
            .unwrap()
            .props
            .push(placed);
        settle(&mut app);
        let visible = jobs::testing::snapshot(app.world(), owner).unwrap();
        assert_eq!(visible.count, 1);
        assert_eq!(visible.meshes.len(), 1);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        let world = app.world_mut();
        let mut roots = world.query::<(&crate::transitions::QueuedChunk, &Visibility)>();
        let queued: Vec<_> = roots.iter(world).collect();
        if mode == terra_core::ChunkAnimation::None {
            assert!(queued.is_empty());
        } else {
            assert_eq!(queued.len(), 1);
            assert_eq!(*queued[0].1, Visibility::Hidden);
        }
    }
}

#[test]
fn prop_lod_replacement_retains_the_old_batch_until_an_atomic_swap() {
    for mode in [
        terra_core::ChunkAnimation::None,
        terra_core::ChunkAnimation::Fade,
    ] {
        use bevy::camera::visibility::VisibilityRange;
        let mut app = app();
        app.world_mut()
            .resource_mut::<terra_core::GameConfig>()
            .client
            .chunk_animation = mode;
        if mode != terra_core::ChunkAnimation::None {
            app.world_mut().spawn((
                terra_player::TerraCamera,
                bevy::camera::primitives::Frustum::default(),
                GlobalTransform::default(),
            ));
        }
        let owner = SectionPos::new(-1, 0, -1);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(owner, section(owner, vec![prop(1, owner), prop(2, owner)]));
        settle(&mut app);
        let near = jobs::testing::snapshot(app.world(), owner).unwrap();
        assert_eq!(app.world().resource::<crate::LodSwapStatus>().completed, 0);

        for (index, focus) in [[208.0, 8.0, 8.0], [8.0, 8.0, 8.0]].into_iter().enumerate() {
            let before = jobs::testing::snapshot(app.world(), owner).unwrap();
            app.world_mut().resource_mut::<ChunkFocus>().0 = focus;
            app.update();
            assert_eq!(jobs::testing::snapshot(app.world(), owner).unwrap(), before);
            assert_eq!(jobs::testing::pending(app.world()), 1);
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                app.update();
                let world = app.world_mut();
                assert_eq!(world.query::<&Mesh3d>().iter(world).count(), 1);
                assert_eq!(world.resource::<Assets<Mesh>>().len(), 1);
                assert!(
                    world
                        .query::<&VisibilityRange>()
                        .iter(world)
                        .all(|range| { range.is_abrupt() && range.end_margin == (192.0..192.0) })
                );
                assert!(world.resource::<PropRenderStatus>().error.is_none());
                if world.resource::<PropRenderStatus>().ready {
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
            let after = jobs::testing::snapshot(app.world(), owner).unwrap();
            if mode != terra_core::ChunkAnimation::None {
                assert!(
                    app.world_mut()
                        .query::<&crate::transitions::QueuedChunk>()
                        .iter(app.world())
                        .count()
                        > 0
                );
            }
            assert_ne!(after.meshes, before.meshes);
            assert_eq!(after.count, near.count);
            assert_eq!(after.terrain_epoch, near.terrain_epoch);
            assert_eq!(after.prop_epoch, near.prop_epoch);
            let mesh = app
                .world()
                .resource::<Assets<Mesh>>()
                .get(after.meshes[0])
                .unwrap();
            let Some(bevy::mesh::VertexAttributeValues::Float32x2(weights)) =
                mesh.attribute(Mesh::ATTRIBUTE_UV_0)
            else {
                panic!("missing wind weights")
            };
            assert_eq!(
                weights
                    .iter()
                    .any(|[height, flexibility]| height * flexibility > 0.0),
                index == 1,
                "distant props retain geometry but skip invisible wind work"
            );
            assert_eq!(
                app.world().resource::<crate::LodSwapStatus>().completed,
                index + 1
            );
        }
        app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
        settle(&mut app);
        assert!(app.world().resource::<Assets<Mesh>>().is_empty());
        let world = app.world_mut();
        assert_eq!(world.query::<&Mesh3d>().iter(world).count(), 0);
    }
}

#[test]
fn vertical_flight_reselects_prop_geometry_without_losing_props() {
    let mut app = app();
    let owner = SectionPos::default();
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(owner, section(owner, vec![prop(1, owner)]));
    app.world_mut().resource_mut::<ChunkFocus>().0 = [8.0, 8.0, 8.0];
    settle(&mut app);
    let before = jobs::testing::snapshot(app.world(), owner).unwrap();
    app.world_mut().resource_mut::<ChunkFocus>().0 = [8.0, 256.0, 8.0];
    settle(&mut app);
    let after = jobs::testing::snapshot(app.world(), owner).unwrap();
    assert_ne!(before.meshes, after.meshes);
    assert_eq!(before.count, after.count);
    assert_eq!(before.prop_epoch, after.prop_epoch);
}

#[test]
fn arrivals_preserve_pending_prop_lod_replacements() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<terra_core::GameConfig>()
        .client
        .max_mesh_jobs = 1;
    let owners: Vec<_> = (0..8).map(|x| SectionPos::new(x, 0, 0)).collect();
    for &owner in &owners {
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(owner, section(owner, vec![prop(1, owner)]));
    }
    settle(&mut app);
    let before: Vec<_> = owners
        .iter()
        .map(|&p| jobs::testing::snapshot(app.world(), p).unwrap().meshes)
        .collect();
    app.world_mut().resource_mut::<ChunkFocus>().0 = [1000.0, 0.0, 0.0];
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        // Streaming continues while only one worker can consume the LOD queue.
        let arrival = SectionPos::new(-20, 0, 0);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(arrival, section(arrival, Vec::new()));
        app.update();
        if owners.iter().zip(&before).all(|(&p, old)| {
            jobs::testing::snapshot(app.world(), p).is_some_and(|s| s.meshes != *old)
        }) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "streaming starved a queued prop LOD"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn prepared_columns_and_distant_roofs_keep_live_prop_handles_without_requeue() {
    use terra_world::streaming::{PreparedColumnChanged, SectionPublished, SectionRemoved};
    let mut app = app();
    app.add_message::<PreparedColumnChanged>()
        .add_message::<SectionPublished>()
        .add_message::<SectionRemoved>();
    app.world_mut()
        .resource_mut::<terra_core::GameConfig>()
        .client
        .chunk_animation = terra_core::ChunkAnimation::None;
    let owner = SectionPos::default();
    let flower = WorldProp {
        kind: PropKind::Flower,
        ..prop(2, owner)
    };
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(owner, section(owner, vec![prop(1, owner), flower]));
    settle(&mut app);
    let before = jobs::testing::snapshot(app.world(), owner).unwrap();
    let completed = app.world().resource::<PropRenderStatus>().completed_jobs;
    let mesh_count = app.world().resource::<Assets<Mesh>>().len();
    let upper = SectionPos::new(0, 3, 0);
    for phase in 0..4 {
        if phase % 2 == 0 {
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .insert(upper, section(upper, Vec::new()));
            app.world_mut()
                .write_message(SectionPublished { position: upper });
        } else {
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .remove(&upper);
            app.world_mut()
                .write_message(SectionRemoved { position: upper });
        }
        app.world_mut()
            .write_message(PreparedColumnChanged { coordinate: [0, 0] });
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_fluid(
                [12, 2, 12],
                terra_world::water::FluidCell {
                    amount: if phase % 2 == 0 { 4 } else { 0 },
                    source: false,
                    flow: [0; 2],
                },
            )
            .unwrap();
        for _ in 0..4 {
            app.update();
            assert_eq!(jobs::testing::pending(app.world()), 0);
            assert_eq!(jobs::testing::snapshot(app.world(), owner).unwrap(), before);
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), mesh_count);
            assert_eq!(
                app.world().resource::<PropRenderStatus>().completed_jobs,
                completed
            );
        }
    }
}

#[test]
fn actual_snow_burial_and_support_edits_still_replace_visible_props() {
    use terra_world::streaming::SectionPublished;
    let mut app = app();
    app.add_message::<SectionPublished>();
    app.world_mut()
        .resource_mut::<terra_core::GameConfig>()
        .client
        .chunk_animation = terra_core::ChunkAnimation::None;
    let owner = SectionPos::default();
    let selected = WorldProp {
        kind: PropKind::Flower,
        ..prop(1, owner)
    };
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(owner, section(owner, vec![selected]));
    settle(&mut app);
    let before = jobs::testing::snapshot(app.world(), owner).unwrap();
    let support = terra_world::props::placement::support(&selected);
    let cap = [support[0], support[1] + 1, support[2]];
    for (cell, material, expected) in [(cap, 5, 0), (cap, 0, 1), (support, 0, 0)] {
        app.world_mut()
            .resource_mut::<EditJournal>()
            .record(cell, material)
            .unwrap();
        let epoch = app.world().resource::<EditJournal>().mesh_epoch(owner);
        let (_, local) = terra_voxel::global_to_section(cell);
        {
            let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
            let section = chunks.loaded.get_mut(&owner).unwrap();
            section.data.set_block(local, material).unwrap();
            section.mesh_epoch = epoch;
        }
        app.world_mut()
            .write_message(SectionPublished { position: owner });
        settle(&mut app);
        let visible = jobs::testing::snapshot(app.world(), owner).unwrap();
        assert_eq!(visible.count, expected);
        assert_eq!(visible.terrain_epoch, epoch);
        assert!(!app.world().resource::<ChunkStore>().loaded[&owner].props[0].removed);
    }
    assert!(
        before
            .meshes
            .iter()
            .all(|id| { !app.world().resource::<Assets<Mesh>>().contains(*id) })
    );
}

#[test]
fn streamed_prop_messages_survive_full_workers_and_preserve_pending_lods_and_stable_batches() {
    use terra_world::streaming::{PreparedColumnChanged, SectionPublished, SectionRemoved};
    let mut app = app();
    app.add_message::<SectionPublished>()
        .add_message::<SectionRemoved>()
        .add_message::<PreparedColumnChanged>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / 60.0),
        ));
    {
        let mut config = app.world_mut().resource_mut::<terra_core::GameConfig>();
        config.client.max_mesh_jobs = 1;
        config.client.chunk_animation = terra_core::ChunkAnimation::None;
    }
    let owners: Vec<_> = (0..5).map(|x| SectionPos::new(x, 0, 0)).collect();
    let stable = SectionPos::new(100, 0, 0);
    for &position in owners.iter().chain(std::iter::once(&stable)) {
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(position, section(position, vec![prop(1, position)]));
    }
    settle(&mut app);
    let before: Vec<_> = owners
        .iter()
        .map(|&position| {
            jobs::testing::snapshot(app.world(), position)
                .unwrap()
                .meshes
        })
        .collect();
    let stable_before = jobs::testing::snapshot(app.world(), stable).unwrap();
    let mut blocked = crate::MeshUploadStatus::default();
    for _ in 0..4 {
        assert!(blocked.reserve(crate::upload_budget::UploadKind::Props, 0));
    }
    app.insert_resource(blocked);
    app.world_mut().resource_mut::<ChunkFocus>().0 = [1000.0, 8.0, 8.0];
    app.update();
    assert_eq!(jobs::testing::pending(app.world()), 1);
    let arrivals: Vec<_> = (-20..-16).map(|x| SectionPos::new(x, 0, 0)).collect();
    for &position in &arrivals {
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(position, section(position, vec![prop(2, position)]));
        app.world_mut().write_message(SectionPublished { position });
        app.world_mut().write_message(PreparedColumnChanged {
            coordinate: [position.x, position.z],
        });
        app.update();
        assert_eq!(jobs::testing::pending(app.world()), 1);
        assert!(jobs::testing::snapshot(app.world(), position).is_none());
    }
    for _ in 0..4 {
        app.update();
    }
    assert!(
        app.world()
            .resource::<Messages<SectionPublished>>()
            .is_empty()
    );
    app.world_mut().remove_resource::<crate::MeshUploadStatus>();
    settle(&mut app);
    for (&position, old) in owners.iter().zip(before) {
        assert_ne!(
            jobs::testing::snapshot(app.world(), position)
                .unwrap()
                .meshes,
            old
        );
    }
    for &position in &arrivals {
        assert_eq!(
            jobs::testing::snapshot(app.world(), position)
                .unwrap()
                .count,
            1
        );
    }
    assert_eq!(
        jobs::testing::snapshot(app.world(), stable).unwrap(),
        stable_before
    );
    let removed = arrivals[0];
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .remove(&removed);
    app.world_mut()
        .write_message(SectionRemoved { position: removed });
    settle(&mut app);
    assert!(jobs::testing::snapshot(app.world(), removed).is_none());
    assert_eq!(
        jobs::testing::snapshot(app.world(), stable).unwrap(),
        stable_before
    );
    let direct = SectionPos::new(-40, 0, 0);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(direct, section(direct, vec![prop(3, direct)]));
    settle(&mut app);
    assert_eq!(
        jobs::testing::snapshot(app.world(), direct).unwrap().count,
        1
    );
    assert_eq!(
        jobs::testing::snapshot(app.world(), stable).unwrap(),
        stable_before
    );
}

#[test]
fn stale_prop_result_retries_without_camera_or_world_changes() {
    let mut app = app();
    let owner = SectionPos::new(-1, 0, -1);
    let props = vec![prop(1, owner), prop(2, owner)];
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(owner, section(owner, props.clone()));
    settle(&mut app);
    let before = jobs::testing::snapshot(app.world(), owner).unwrap();
    let completed = app.world().resource::<PropRenderStatus>().completed_jobs;
    jobs::testing::enqueue_stale_lod(app.world_mut(), owner, props);
    settle(&mut app);
    let after = jobs::testing::snapshot(app.world(), owner).unwrap();
    assert_eq!(after.count, before.count);
    assert_ne!(after.meshes, before.meshes);
    assert_eq!(
        app.world().resource::<PropRenderStatus>().completed_jobs,
        completed + 2,
        "the stale worker and its current replacement must both finish"
    );
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
}

#[test]
fn repeated_prop_streaming_and_lod_changes_keep_mesh_and_material_budgets() {
    let mut app = app();
    settle(&mut app);
    let mut entities = app.world_mut().query::<Entity>();
    let baseline_entities = entities.iter(app.world()).count();
    let standard_materials = app.world().resource::<Assets<StandardMaterial>>().len();
    let owner = SectionPos::new(-1, 0, -1);
    for cycle in 0..50 {
        let props = [
            PropKind::Grass,
            PropKind::Flower,
            PropKind::Bush,
            PropKind::Fern,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, kind)| WorldProp {
            kind,
            ..prop(index as u64 + 1, owner)
        })
        .collect();
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(owner, section(owner, props));
        let mut previous = Vec::new();
        for focus in [[-8.0, 8.0, -8.0], [120.0, 8.0, -8.0], [512.0, 8.0, -8.0]] {
            app.world_mut().resource_mut::<ChunkFocus>().0 = focus;
            settle(&mut app);
            let current = jobs::testing::snapshot(app.world(), owner).unwrap();
            assert_eq!(current.count, 4);
            assert_eq!(current.meshes.len(), 1);
            assert_eq!(
                app.world().resource::<Assets<Mesh>>().len(),
                1,
                "cycle {cycle}"
            );
            assert_eq!(entities.iter(app.world()).count(), baseline_entities + 2);
            assert_eq!(app.world().resource::<Assets<PropMaterial>>().len(), 1);
            assert_eq!(
                app.world().resource::<Assets<StandardMaterial>>().len(),
                standard_materials
            );
            assert!(
                previous
                    .iter()
                    .all(|id| app.world().resource::<Assets<Mesh>>().get(*id).is_none())
            );
            previous = current.meshes;
        }
        app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
        settle(&mut app);
        assert!(jobs::testing::snapshot(app.world(), owner).is_none());
        assert_eq!(jobs::testing::pending(app.world()), 0);
        assert!(app.world().resource::<Assets<Mesh>>().is_empty());
        assert_eq!(
            entities.iter(app.world()).count(),
            baseline_entities,
            "cycle {cycle}"
        );
        assert_eq!(app.world().resource::<PropRenderStatus>().batches, 0);
        assert_eq!(app.world().resource::<PropRenderStatus>().visible_props, 0);
        assert_eq!(app.world().resource::<Assets<PropMaterial>>().len(), 1);
    }
}

#[test]
fn empty_prop_batches_retire_previous_mesh_without_uploading_empty_buffers() {
    let mut app = app();
    let owner = SectionPos::new(-1, 0, -1);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(owner, section(owner, vec![prop(1, owner)]));
    settle(&mut app);
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
    jobs::testing::enqueue_empty_result(app.world_mut(), owner);
    settle(&mut app);
    assert!(app.world().resource::<Assets<Mesh>>().is_empty());
    assert_eq!(
        app.world_mut().query::<&Mesh3d>().iter(app.world()).count(),
        0
    );
    assert_eq!(app.world().resource::<PropRenderStatus>().batches, 0);
    assert_eq!(app.world().resource::<PropRenderStatus>().visible_props, 0);
    assert_eq!(
        jobs::testing::snapshot(app.world(), owner).unwrap().count,
        0
    );
}

#[test]
fn transformed_geometry_fits_picking_bounds_for_every_species() {
    for kind in [
        PropKind::Grass,
        PropKind::Flower,
        PropKind::Bush,
        PropKind::Fern,
        PropKind::Mushroom,
        PropKind::SmallPlant,
    ] {
        for variant in 0..=u8::MAX {
            let template_variant = variant % 8 + if variant >= 128 { 128 } else { 0 };
            for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
                let geometry =
                    shapes::template(kind, template_variant, level, &PropMaterials::default())
                        .unwrap();
                for angle in [0.0, 0.5, 1.3, 2.4] {
                    let prop = WorldProp {
                        kind,
                        variant,
                        rotation: angle,
                        ..prop(1, SectionPos::default())
                    };
                    let [low, high] = prop.bounds();
                    for vertex in geometry.iter().flat_map(|batch| &batch.positions) {
                        let vertex = Quat::from_rotation_y(angle)
                            * Vec3::from_array(*vertex)
                            * prop.visual_scale();
                        for axis in 0..3 {
                            let coordinate = f64::from(vertex[axis]) + prop.position[axis];
                            assert!(
                                (low[axis] - 0.002..=high[axis] + 0.002).contains(&coordinate),
                                "{kind:?}/{variant} axis {axis}: {coordinate} outside {:?}",
                                prop.bounds()
                            );
                        }
                    }
                }
            }
        }
    }
}

fn decode_prop_color(color: [u8; 4]) -> [f32; 4] {
    [
        f32::from(color[0]) / 255.0 * 2.0,
        f32::from(color[1]) / 255.0 * 2.0,
        f32::from(color[2]) / 255.0 * 2.0,
        f32::from(color[3]) / 255.0,
    ]
}

fn colors(world: &World, section: SectionPos) -> Vec<[f32; 4]> {
    let id = jobs::testing::snapshot(world, section).unwrap().meshes[0];
    let mesh = world.resource::<Assets<Mesh>>().get(id).unwrap();
    match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
        Some(bevy::mesh::VertexAttributeValues::Unorm8x4(colors)) => {
            colors.iter().copied().map(decode_prop_color).collect()
        }
        Some(bevy::mesh::VertexAttributeValues::Float32x4(colors)) => colors.clone(),
        _ => panic!("missing prop colors"),
    }
}

fn expected_packed_prop_colors(
    world: &World,
    prop: &WorldProp,
    condition: terra_world::environment::SurfaceCondition,
) -> Vec<[f32; 4]> {
    let blocks = world.resource::<crate::blocks::BlockAssets>();
    let materials = world.resource::<Assets<StandardMaterial>>();
    let palette = &blocks.templates.as_ref().unwrap().prop_materials;
    let origin = prop.section.origin().unwrap();
    let biome = crate::appearance::GrassTint::new(&world.resource::<WorldTerrain>().0, origin)
        .relative_at(std::array::from_fn(|axis| {
            (prop.position[axis] - origin[axis] as f64) as f32
        }));
    let tint = crate::appearance::condition_tint(condition);
    let mut expected = Vec::new();
    for part in shapes::template(
        prop.kind,
        prop.template_variant(),
        DetailLevel::Near,
        palette,
    )
    .unwrap()
    {
        let source = materials
            .get(&blocks.materials[part.material])
            .unwrap()
            .base_color
            .to_linear()
            .to_f32_array();
        let green = prop.kind != PropKind::Mushroom
            && prop.kind.crop().is_none()
            && (palette.foliage.contains(&part.material) || part.material == palette.stem);
        for vertex in 0..part.positions.len() {
            let shade = part.colors.get(vertex).copied().unwrap_or([1.0; 4]);
            let mut color = std::array::from_fn(|channel| source[channel] * shade[channel]);
            for channel in 0..3 {
                color[channel] *= tint[channel] * if green { biome[channel] } else { 1.0 };
            }
            color[3] = terra_world::props::seasonal::flower_rank(prop);
            expected.push(decode_prop_color(crate::foliage::packing::color(color)));
        }
    }
    expected
}

#[test]
fn local_conditions_rebuild_only_owner_and_persist_at_negative_section_ceiling() {
    use terra_world::environment::SurfaceCondition;
    let mut app = app();
    let owner = SectionPos::new(-1, -2, -1);
    let neighbor = SectionPos::new(0, -2, -1);
    let mut selected = prop(1, owner);
    selected.position[1] = -16.0;
    let original = vec![selected, prop(2, owner)];
    let upper = owner.offset([0, 1, 0]).unwrap();
    let mut root_space = section(upper, Vec::new());
    root_space.data = BlockSection::empty(upper).unwrap();
    app.world_mut().resource_mut::<ChunkStore>().loaded.extend([
        (owner, section(owner, original.clone())),
        (neighbor, section(neighbor, vec![prop(3, neighbor)])),
        (upper, root_space),
    ]);
    settle(&mut app);
    let before = jobs::testing::snapshot(app.world(), owner).unwrap();
    let next_before = jobs::testing::snapshot(app.world(), neighbor).unwrap();
    let before_colors = colors(app.world(), owner);
    let expected_before: Vec<_> = original
        .iter()
        .flat_map(|prop| expected_packed_prop_colors(app.world(), prop, Default::default()))
        .collect();
    assert_eq!(before_colors, expected_before);
    let support = [
        selected.position[0].floor() as i64,
        -17,
        selected.position[2].floor() as i64,
    ];
    let condition = SurfaceCondition::new(0.6, 0.8).unwrap();
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_surface_condition(support, condition)
        .unwrap();
    app.update();
    assert_eq!(
        jobs::testing::snapshot(app.world(), owner).unwrap(),
        before,
        "existing geometry stays visible while the appearance worker runs"
    );
    settle(&mut app);
    let after = jobs::testing::snapshot(app.world(), owner).unwrap();
    assert_eq!(after.appearance_epoch, 1);
    assert_eq!(after.terrain_epoch, before.terrain_epoch);
    assert_eq!(after.prop_epoch, before.prop_epoch);
    assert_eq!(after.count, before.count);
    assert_ne!(after.meshes, before.meshes);
    assert_eq!(
        jobs::testing::snapshot(app.world(), neighbor).unwrap(),
        next_before
    );
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
    let after_colors = colors(app.world(), owner);
    let expected_after: Vec<_> = original
        .iter()
        .flat_map(|prop| {
            expected_packed_prop_colors(
                app.world(),
                prop,
                if prop.id == selected.id {
                    condition
                } else {
                    Default::default()
                },
            )
        })
        .collect();
    assert_eq!(after_colors, expected_after);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .remove(&owner);
    settle(&mut app);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(owner, section(owner, original));
    settle(&mut app);
    assert_eq!(colors(app.world(), owner), after_colors);
    assert_eq!(
        jobs::testing::snapshot(app.world(), owner)
            .unwrap()
            .appearance_epoch,
        1
    );
}

#[test]
fn stale_appearance_job_cannot_replace_visible_batch() {
    use terra_world::environment::SurfaceCondition;
    let mut app = app();
    let owner = SectionPos::default();
    let selected = prop(1, owner);
    let props = vec![selected];
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(owner, section(owner, props.clone()));
    settle(&mut app);
    let original = jobs::testing::snapshot(app.world(), owner).unwrap();
    jobs::testing::enqueue(app.world_mut(), owner, 0, 0, props);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_surface_condition(
            [
                selected.position[0].floor() as i64,
                0,
                selected.position[2].floor() as i64,
            ],
            SurfaceCondition::new(1.0, 0.0).unwrap(),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        let visible = jobs::testing::snapshot(app.world(), owner).unwrap();
        if visible.appearance_epoch == 1 {
            break;
        }
        assert_eq!(visible, original, "stale appearance batch became visible");
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn weather_and_view_distance_update_existing_batches_without_mesh_jobs() {
    use bevy::camera::visibility::VisibilityRange;
    use terra_core::{GameConfig, WeatherKind};
    let mut app = app();
    let owner = SectionPos::default();
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(owner, section(owner, vec![prop(1, owner)]));
    let unrelated = app
        .world_mut()
        .spawn(VisibilityRange::abrupt(0.0, 27.0))
        .id();
    settle(&mut app);
    let before = jobs::testing::snapshot(app.world(), owner).unwrap();
    let completed = app.world().resource::<PropRenderStatus>().completed_jobs;
    let handle = app
        .world_mut()
        .query::<&MeshMaterial3d<PropMaterial>>()
        .single(app.world())
        .unwrap()
        .0
        .clone();
    let material_count = app.world().resource::<Assets<PropMaterial>>().len();
    for (weather, intensity, distance) in [
        (WeatherKind::Rain, 0.75, 10),
        (WeatherKind::Snow, 1.0, 16),
        (WeatherKind::Clear, 1.0, 12),
    ] {
        {
            let mut config = app.world_mut().resource_mut::<GameConfig>();
            config.client.weather = weather;
            config.client.weather_intensity = intensity;
            config.client.view_distance = distance;
        }
        settle(&mut app);
        assert_eq!(jobs::testing::snapshot(app.world(), owner).unwrap(), before);
        assert_eq!(
            app.world().resource::<PropRenderStatus>().completed_jobs,
            completed
        );
        assert_eq!(
            app.world().resource::<Assets<PropMaterial>>().len(),
            material_count
        );
        let [r, g, b] = crate::appearance::weather_tint(weather, intensity);
        let material = app
            .world()
            .resource::<Assets<PropMaterial>>()
            .get(&handle)
            .unwrap();
        assert_eq!(
            material.base.base_color.to_linear(),
            Color::WHITE.to_linear()
        );
        assert_eq!(
            app.world()
                .resource::<crate::weather::storage::WeatherPayload>()
                .0
                .tint,
            Vec4::new(r, g, b, 0.0),
            "RGB carries weather tint; default spring has zero temperature offset in W"
        );
        let world = app.world_mut();
        let end = f32::from(distance) * 16.0;
        for (entity, range) in world.query::<(Entity, &VisibilityRange)>().iter(world) {
            assert!(range.is_abrupt());
            assert_eq!(
                range.end_margin,
                if entity == unrelated {
                    27.0..27.0
                } else {
                    end..end
                }
            );
        }
    }

    app.init_resource::<terra_core::WorldClock>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_millis(100),
        ));
    for (day, temperature_offset) in [(7, 6.0), (21, -16.0), (0, 0.0)] {
        app.world_mut().resource_mut::<terra_core::WorldClock>().day = day;
        for _ in 0..60 {
            app.update();
        }
        let tint = app
            .world()
            .resource::<crate::weather::storage::WeatherPayload>()
            .0
            .tint;
        assert_eq!(tint.truncate(), Vec3::ONE);
        assert!(
            (tint.w - temperature_offset).abs() < 0.01,
            "temperature offset must settle through the shared payload: {tint:?}"
        );
        assert_eq!(jobs::testing::snapshot(app.world(), owner).unwrap(), before);
        assert_eq!(
            app.world().resource::<PropRenderStatus>().completed_jobs,
            completed
        );
        assert_eq!(
            app.world().resource::<Assets<PropMaterial>>().len(),
            material_count
        );
    }
}

#[test]
fn plant_wind_stays_inside_pose_expanded_pick_bounds_at_all_lods() {
    for kind in [
        PropKind::Grass,
        PropKind::Bush,
        PropKind::Flower,
        PropKind::Fern,
        PropKind::SmallPlant,
        PropKind::Mushroom,
    ] {
        for variant in 0..=u8::MAX {
            let template_variant = variant % 8 + if variant >= 128 { 128 } else { 0 };
            for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
                let geometry =
                    shapes::template(kind, template_variant, level, &PropMaterials::default())
                        .unwrap();
                let wind_strength = match level {
                    DetailLevel::Near => 1.0,
                    DetailLevel::Middle => 0.5,
                    DetailLevel::Far => 0.0,
                };
                for angle in [0.0, 0.5, 1.3, 2.4] {
                    let plant = WorldProp {
                        kind,
                        variant,
                        rotation: angle,
                        scale: 1.0,
                        position: [-17.5, 16.0, -0.5],
                        ..prop(1, SectionPos::default())
                    };
                    let bounds = plant.bounds();
                    let margin = f64::from(
                        (wind::WIND_AMPLITUDE + wind::WIND_FLUTTER) * plant.visual_scale(),
                    );
                    for point in geometry.iter().flat_map(|batch| &batch.positions) {
                        let [height, flexibility] =
                            wind::parameters(kind, variant, point[1], plant.visual_scale());
                        let maximum = height
                            * flexibility
                            * wind_strength
                            * (wind::WIND_AMPLITUDE + wind::WIND_FLUTTER);
                        let point = Quat::from_rotation_y(angle)
                            * Vec3::from_array(*point)
                            * plant.visual_scale();
                        // Exercise both extrema of the shader's bounded sway, which
                        // covers every animation time and the server's swept picking.
                        for sign in [-1.0, 1.0] {
                            let moved = point + Vec3::new(maximum, 0.0, maximum * 0.6) * sign;
                            for axis in 0..3 {
                                let coordinate = plant.position[axis] + f64::from(moved[axis]);
                                let margin = if axis == 1 { 0.0 } else { margin };
                                assert!(
                                    (bounds[0][axis] - margin - 1e-6
                                        ..=bounds[1][axis] + margin + 1e-6)
                                        .contains(&coordinate),
                                    "{kind:?}/{variant}/{level:?} angle {angle} axis {axis}"
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
fn grass_lods_retain_the_same_prominent_blades() {
    for variant in 0..=u8::MAX {
        let shapes = [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far].map(|level| {
            shapes::template(PropKind::Grass, variant, level, &PropMaterials::default()).unwrap()
        });
        for blade in 0..4 {
            assert_eq!(
                &shapes[0][0].positions[blade * 20 + 16..blade * 20 + 20],
                &shapes[1][0].positions[blade * 20 + 16..blade * 20 + 20]
            );
            assert_eq!(
                shapes[0][0].positions[blade * 20],
                shapes[1][0].positions[blade * 20]
            );
        }
        assert_eq!(shapes[2][0].positions.len(), 32);
        for blade in 0..4 {
            let near = &shapes[1][0].positions[blade * 20..blade * 20 + 4];
            let far = &shapes[2][0].positions[blade * 8..blade * 8 + 4];
            let center = |points: &[[f32; 3]], a: usize, b: usize| {
                (Vec3::from_array(points[a]) + Vec3::from_array(points[b])) * 0.5
            };
            let near_root = (Vec3::from_array(shapes[1][0].positions[blade * 20])
                + Vec3::from_array(shapes[1][0].positions[blade * 20 + 8]))
                * 0.5;
            assert!(center(far, 0, 3).distance(near_root) < 1e-6);
            assert!((far[1][1] - near[1][1]).abs() < 1e-6);
            assert!((far[2][1] - near[2][1]).abs() < 1e-6);
        }
        assert!(shapes[0][0].positions.iter().all(|point| point[1] >= 0.0));
    }
}

#[test]
fn fine_bushes_have_connected_cubic_crowns_and_bounded_berry_details() {
    let palette = PropMaterials {
        foliage: [0; 3],
        twig: 1,
        flowers: [2; 8],
        ..default()
    };
    for variant in 0..128 {
        let shape = shapes::template(PropKind::Bush, variant, DetailLevel::Near, &palette).unwrap();
        let leaves = shape.iter().find(|batch| batch.material == 0).unwrap();
        let branches = shape.iter().find(|batch| batch.material == 1).unwrap();
        assert_eq!(leaves.positions.len(), 69 * 24);
        assert_eq!(branches.positions.len(), 24);
        let mut tiers = std::collections::HashSet::new();
        for leaf in leaves.positions.chunks_exact(24) {
            let minimum = leaf.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
            let maximum = leaf.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
            assert!(
                (0.035..0.25).contains(&(maximum - minimum)),
                "small leaf segments retain real voxel volume"
            );
            let center = (minimum + maximum) * 0.5;
            tiers.insert((center * 100.0).round() as i32);
        }
        assert!(tiers.len() >= 4);
    }
}

#[test]
fn desert_variants_use_dry_grass_and_bare_connected_twigs() {
    let palette = PropMaterials {
        foliage: [0, 1, 2],
        stem: 3,
        dry_grass: [4, 5, 6],
        twig: 7,
        ..default()
    };
    for variant in 128..136 {
        for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
            let grass = shapes::template(PropKind::Grass, variant, level, &palette).unwrap();
            assert!(
                grass
                    .iter()
                    .all(|batch| palette.dry_grass.contains(&batch.material))
            );
            let bush = shapes::template(PropKind::Bush, variant, level, &palette).unwrap();
            assert_eq!(bush.len(), 1);
            assert_eq!(bush[0].material, palette.twig);
            assert!(bush[0].positions.len() >= 7 * 24);
            assert!(
                wind::parameters(PropKind::Bush, variant, 0.5, 1.0)[1]
                    < wind::parameters(PropKind::Bush, 0, 0.5, 1.0)[1]
            );
        }
    }
}

#[test]
fn small_mushrooms_match_the_six_giant_cap_colors() {
    let palette = PropMaterials {
        foliage: [0, 1, 2],
        stem: 3,
        flowers: [4, 5, 6, 7, 8, 9, 16, 17],
        mushroom: 10,
        twig: 11,
        ..default()
    };
    for (variant, cap) in [9, 10, 5, 1, 7, 6].into_iter().enumerate() {
        let shape = shapes::template(
            PropKind::Mushroom,
            variant as u8,
            DetailLevel::Near,
            &palette,
        )
        .unwrap();
        let cap = shape.iter().find(|batch| batch.material == cap).unwrap();
        assert!(cap.positions.iter().any(|position| position[1] >= 0.36));
        if variant == 5 {
            assert!(shape.iter().any(|batch| batch.material == palette.twig));
        }
    }
}

#[test]
fn plant_roots_stay_fixed_and_non_plants_do_not_receive_wind() {
    for kind in [
        PropKind::Grass,
        PropKind::Bush,
        PropKind::Flower,
        PropKind::Fern,
        PropKind::SmallPlant,
    ] {
        for scale in [0.5, 1.0, 1.4] {
            assert_eq!(wind::parameters(kind, 0, 0.0, scale)[0], 0.0);
            assert_eq!(wind::parameters(kind, 0, -0.01, scale)[0], 0.0);
            assert!(
                wind::parameters(kind, 0, 0.5, scale)
                    .into_iter()
                    .all(|v| v > 0.0)
            );
        }
    }
    for kind in [PropKind::Mushroom, PropKind::SmallPlant] {
        for height in [0.0, 0.2, 0.5, 1.0] {
            assert_eq!(wind::parameters(kind, 128, height, 1.0), [0.0; 2]);
        }
    }
}

#[test]
fn shared_wind_material_has_conservative_bounds_and_never_rebuilds_settled_props() {
    use bevy::{camera::primitives::Aabb, mesh::VertexAttributeValues};
    let mut app = app();
    let owner = SectionPos::default();
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        owner,
        section(
            owner,
            vec![
                prop(1, owner),
                WorldProp {
                    kind: PropKind::Grass,
                    ..prop(2, owner)
                },
            ],
        ),
    );
    settle(&mut app);
    let snapshot = jobs::testing::snapshot(app.world(), owner).unwrap();
    let completed = app.world().resource::<PropRenderStatus>().completed_jobs;
    let bounds = *app
        .world_mut()
        .query_filtered::<&Aabb, With<WindProp>>()
        .single(app.world())
        .unwrap();
    let mesh = app
        .world()
        .resource::<Assets<Mesh>>()
        .get(snapshot.meshes[0])
        .unwrap();
    let Some(VertexAttributeValues::Float32x3(points)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("missing positions")
    };
    let Some(VertexAttributeValues::Float32x2(weights)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
    else {
        panic!("missing wind weights")
    };
    for (point, [height, flexibility]) in points.iter().zip(weights) {
        let maximum = height * flexibility * (wind::WIND_AMPLITUDE + wind::WIND_FLUTTER);
        for sign in [-1.0, 1.0] {
            let moved = Vec3::from_array(*point) + Vec3::new(maximum, 0.0, maximum * 0.6) * sign;
            assert!(
                (moved - Vec3::from(bounds.center))
                    .abs()
                    .cmple(Vec3::from(bounds.half_extents) + Vec3::splat(1e-5))
                    .all()
            );
        }
    }
    let changed = jobs::testing::changed_tick(app.world());
    for _ in 0..12 {
        app.update();
    }
    assert_eq!(
        jobs::testing::changed_tick(app.world()),
        changed,
        "idle upload bookkeeping must not keep invalidating the render cache"
    );
    assert_eq!(
        jobs::testing::snapshot(app.world(), owner).unwrap(),
        snapshot
    );
    assert_eq!(
        app.world().resource::<PropRenderStatus>().completed_jobs,
        completed
    );
    assert_eq!(app.world().resource::<Assets<PropMaterial>>().len(), 1);
}
