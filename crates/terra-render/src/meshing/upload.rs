use super::{
    RenderStatus, RenderedSection, RenderedSections, TerrainSection,
    jobs::{RenderJobs, poll_job},
};
use crate::blocks::BlockAssets;
use bevy::ecs::system::SystemParam;
use bevy::{
    asset::RenderAssetUsages, camera::primitives::MeshAabb, math::Vec3A, mesh::PrimitiveTopology,
    prelude::*,
};
use terra_world::edits::EditJournal;
use terra_world::streaming::ChunkStore;

#[derive(SystemParam)]
pub(crate) struct RenderWorld<'w, 's> {
    cache: Option<Res<'w, super::GeometryCache>>,
    budget: Option<ResMut<'w, crate::MeshUploadStatus>>,
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    focus: Option<Res<'w, terra_world::streaming::ChunkFocus>>,
    config: Option<Res<'w, terra_core::GameConfig>>,
    graphics: Option<Res<'w, terra_core::GraphicsConfig>>,
    foliage: Option<Res<'w, crate::foliage::FoliageAssets>>,
    water: Option<Res<'w, crate::water::material::WaterAssets>>,
    wet: Option<Res<'w, crate::weather::surfaces::WetMaterials>>,
    terrain: Option<Res<'w, terra_world::streaming::WorldTerrain>>,
    entering: Query<'w, 's, (), With<crate::transitions::ChunkFade>>,
    queued: Query<'w, 's, (), With<crate::transitions::QueuedChunk>>,
}

pub(crate) fn upload_sections(
    mut commands: Commands,
    mut world: RenderWorld,
    mut rendered: ResMut<RenderedSections>,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Res<BlockAssets>,
    mut jobs: ResMut<RenderJobs>,
    mut status: ResMut<RenderStatus>,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("terrain.upload");
    if jobs.0.is_empty() && !world.chunks.is_changed() && !rendered.is_changed() {
        return;
    }
    let chunks = &world.chunks;
    let animation = world
        .config
        .as_ref()
        .map_or(terra_core::ChunkAnimation::None, |c| {
            c.client.chunk_animation
        });
    if world.chunks.is_changed() {
        rendered
            .block_revisions
            .retain(|p, _| chunks.loaded.contains_key(p));
        let removed: Vec<_> = rendered
            .sections
            .keys()
            .copied()
            .filter(|p| !chunks.loaded.contains_key(p))
            .collect();
        for position in removed {
            if let Some(old) = rendered.sections.remove(&position) {
                crate::transitions::retire(&mut commands, old.entity, old.meshes, animation);
            }
        }
    }
    let mut completed = Vec::new();
    let focus = world.focus.as_ref().map_or([0.0; 3], |f| f.0);
    let mut positions: Vec<_> = jobs.0.keys().copied().collect();
    positions.sort_by_key(|p| {
        (
            super::lod::distance_squared(*p, focus) as u64,
            p.y,
            p.z,
            p.x,
        )
    });
    for position in positions {
        if completed.len()
            == world
                .config
                .as_ref()
                .map_or(12, |c| c.client.mesh_uploads_per_frame)
        {
            break;
        }
        let Some(job) = jobs.0.get_mut(&position) else {
            continue;
        };
        if let Some((mut result, seconds)) = poll_job(job) {
            if !super::shadows::enabled(world.graphics.as_deref())
                && let Ok(batches) = &mut result
            {
                batches.retain(|batch| !batch.shadow_only);
            }
            let vertices = result
                .as_ref()
                .map_or(0, |batches| batches.iter().map(|b| b.positions.len()).sum());
            if world
                .budget
                .as_mut()
                .is_some_and(|b| !b.reserve(crate::upload_budget::UploadKind::Terrain, vertices))
            {
                job.ready = Some((result, seconds));
                break;
            }
            status.completed_mesh_jobs += 1;
            status.composition_seconds += seconds;
            status.max_composition_seconds = status.max_composition_seconds.max(seconds);
            completed.push((
                position,
                job.epoch,
                job.appearance_epoch,
                job.level,
                job.micro_detail,
                result,
            ));
        }
    }
    for (position, epoch, appearance_epoch, level, micro_detail, result) in completed {
        jobs.0.remove(&position);
        if rendered.sections.get(&position).is_some_and(|old| {
            world.entering.contains(old.entity)
                && old.mesh_epoch == epoch
                && old.appearance_epoch == appearance_epoch
        }) {
            continue;
        }
        if epoch != world.journal.mesh_epoch(position)
            || appearance_epoch
                != world.cache.as_ref().map_or_else(
                    || world.journal.appearance_epoch(position),
                    |cache| cache.appearance_epoch(position, &world.journal),
                )
            || world.focus.as_ref().is_some_and(|f| {
                if super::lod::select_configured_micro_detail(
                    position,
                    f.0,
                    rendered.sections.get(&position).map(|r| r.micro_detail),
                    world
                        .config
                        .as_ref()
                        .map_or_else(Default::default, |c| c.client.blocks),
                ) != micro_detail
                {
                    return true;
                }
                super::lod::select_level(
                    position,
                    f.0,
                    rendered.sections.get(&position).map(|r| r.detail_level),
                    &world
                        .config
                        .as_ref()
                        .map_or_else(Default::default, |c| c.client.clone()),
                ) != level
            })
            || chunks
                .loaded
                .get(&position)
                .is_none_or(|s| s.mesh_epoch != epoch)
        {
            jobs.1.push(position);
            continue;
        }
        let mut batches = match result {
            Ok(value) => value,
            Err(error) => {
                tracing::error!(%error, "Section composition failed");
                status.error = Some(error);
                continue;
            }
        };
        // Bevy's mesh allocator must not receive an empty vertex/index buffer.
        // Empty section results still retire an obsolete mesh below.
        batches.retain(|batch| !batch.positions.is_empty() && batch.has_indices());
        // A newly generated neighbor can invalidate this section while its
        // first mesh is still queued/fading. That is still an initial reveal,
        // even with a newer epoch; never expose its replacement fully opaque.
        // Building is an immediate world update, not another generated-chunk
        // entrance. This also covers the first block in a previously empty section.
        let revision = world.journal.revision(position);
        let edited = rendered
            .block_revisions
            .insert(position, revision)
            .is_some_and(|previous| previous != revision);
        let first_upload = !edited
            && rendered.sections.get(&position).is_none_or(|old| {
                world.queued.contains(old.entity) || world.entering.contains(old.entity)
            });
        if let Some(old) = rendered.sections.remove(&position) {
            if old.mesh_epoch == epoch && old.detail_level != level {
                crate::transitions::record_swap(&mut commands);
            }
            // Pending and stale jobs leave the old root intact. A valid result
            // replaces both roots in this system's single deferred command commit.
            remove_section(old, &mut commands);
        }
        if batches.is_empty() {
            continue;
        }
        let Ok(origin) = position.origin() else {
            continue;
        };
        let root = commands
            .spawn((
                Name::new(format!(
                    "Section {}, {}, {}",
                    position.x, position.y, position.z
                )),
                TerrainSection {
                    position,
                    mesh_epoch: epoch,
                    detail_level: level,
                },
                Transform::from_translation(Vec3::from_array(origin.map(|v| v as f32))),
                Visibility::default(),
            ))
            .id();
        if let Some((geometry, _)) = world
            .cache
            .as_ref()
            .and_then(|cache| cache.snapshot(position, epoch))
        {
            let mut drips = Vec::new();
            for part in &geometry.segments {
                if part.material != 51 {
                    continue;
                }
                let global = std::array::from_fn(|a| origin[a] + i64::from(part.cell[a]));
                let profile = terra_world::segments::profile(global, 51, |p| chunks.block_at(p));
                let tip = profile.below == 0
                    && (part.below == 0
                        || chunks.block_at([global[0], global[1] - 1, global[2]]) == Some(0));
                if !(profile.hanging || part.hanging) || !tip || chunks.block_at(global) != Some(51)
                {
                    continue;
                }
                let point = Vec3::new(
                    f32::from(part.cell[0]) + 0.5,
                    f32::from(part.cell[1]) + 0.02,
                    f32::from(part.cell[2]) + 0.5,
                );
                drips.push(point.extend(-6.0));
                if drips.len() == 8 {
                    break;
                }
            }
            if !drips.is_empty() {
                commands
                    .entity(root)
                    .insert(crate::water::spray::SpraySources(drips));
            }
        }
        let shadowed_materials: std::collections::HashSet<_> = batches
            .iter()
            .filter(|batch| batch.shadow_only)
            .flat_map(|batch| batch.shadow_sources.iter().copied())
            .collect();
        let mut handles = Vec::with_capacity(batches.len());
        let water_tint = world
            .terrain
            .as_ref()
            .filter(|_| {
                world.water.is_some()
                    && assets.templates.as_ref().is_some_and(|templates| {
                        batches
                            .iter()
                            .any(|batch| templates.water_material(batch.material))
                    })
            })
            .map(|terrain| crate::water::biome::WaterTint::new(&terrain.0, origin));
        for mut batch in batches {
            let shadow_only = batch.shadow_only;
            let rooted_fibre = !batch.rooted_wind.is_empty();
            let damp = !batch.surface_water.is_empty() && !rooted_fibre && !shadow_only;
            let water = world.water.as_ref().filter(|_| {
                assets
                    .templates
                    .as_ref()
                    .is_some_and(|t| t.water_material(batch.material))
            });
            let coastal_quads = if water.is_some() {
                batch
                    .colors
                    .chunks_exact(4)
                    .filter(|quad| (quad[0][3] - 6.0).abs() < 0.001)
                    .count()
            } else {
                0
            };
            let biome_tints = water.and(water_tint.as_ref()).map(|tint| {
                batch
                    .positions
                    .iter()
                    .map(|&p| tint.at(p))
                    .collect::<Vec<_>>()
            });
            let indices = batch.take_indices();
            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, batch.positions)
            .with_inserted_indices(indices);
            if !batch.uvs.is_empty() {
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, batch.uvs);
            }
            if rooted_fibre {
                insert_rooted_wind(&mut mesh, batch.rooted_wind);
            } else if damp {
                let mut values = batch.surface_water;
                values.resize(mesh.count_vertices(), [0.0; 2]);
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, values);
            }
            if let Some(tints) = biome_tints {
                mesh.insert_attribute(crate::water::biome::ATTRIBUTE_BIOME_TINT, tints);
            }
            if !batch.packed_normals.is_empty() {
                mesh.insert_attribute(
                    bevy::mesh::MeshVertexAttribute {
                        format: bevy::render::render_resource::VertexFormat::Snorm8x4,
                        ..Mesh::ATTRIBUTE_NORMAL
                    },
                    bevy::mesh::VertexAttributeValues::Snorm8x4(batch.packed_normals),
                );
            } else if !batch.normals.is_empty() {
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, batch.normals);
            }
            if !batch.packed_colors.is_empty() {
                mesh.insert_attribute(
                    bevy::mesh::MeshVertexAttribute {
                        format: bevy::render::render_resource::VertexFormat::Unorm8x4,
                        ..Mesh::ATTRIBUTE_COLOR
                    },
                    bevy::mesh::VertexAttributeValues::Unorm8x4(batch.packed_colors),
                );
            } else if !batch.colors.is_empty() {
                mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, batch.colors);
            }
            let foliage = world
                .foliage
                .as_ref()
                .and_then(|a| a.materials.get(&batch.material));
            let flame = world
                .foliage
                .as_ref()
                .and_then(|a| a.flames.get(&batch.material));
            let fibre = world
                .foliage
                .as_ref()
                .filter(|_| rooted_fibre)
                .and_then(|assets| assets.fibres.get(&batch.material));
            let bounds = foliage
                .or(fibre)
                .or(flame)
                .and_then(|_| mesh.compute_aabb())
                .map(|mut bounds| {
                    bounds.half_extents += Vec3A::splat(crate::foliage::WIND_BOUNDS);
                    bounds
                });
            let handle = meshes.add(mesh);
            let mut entity =
                commands.spawn((Mesh3d(handle.clone()), Transform::default(), ChildOf(root)));
            if shadow_only {
                entity.insert((
                    super::shadows::ShadowProxy,
                    bevy::camera::visibility::RenderLayers::layer(super::shadows::LAYER),
                ));
            } else {
                entity.insert((
                    crate::miniature::layers(),
                    terra_core::miniature::MiniatureMesh::Terrain,
                ));
            }
            if let Some(water) = water {
                if coastal_quads > 0 {
                    entity.insert(super::CoastalWashQuads(coastal_quads));
                }
                entity.insert((
                    MeshMaterial3d(water.0.clone()),
                    bevy::light::NotShadowCaster,
                ));
            } else if let Some(material) = flame {
                entity.insert((
                    MeshMaterial3d(material.clone()),
                    bevy::light::NotShadowCaster,
                ));
                if let Some(bounds) = bounds {
                    entity.insert(bounds);
                }
            } else if let Some(material) = world
                .foliage
                .as_ref()
                .and_then(|a| a.reference.get(&batch.material))
            {
                entity.insert(MeshMaterial3d(material.clone()));
                if let Some(bounds) = bounds {
                    entity.insert(bounds);
                }
            } else if let Some(material) = foliage {
                entity.insert((
                    MeshMaterial3d(material.clone()),
                    crate::foliage::WindFoliage,
                ));
                if let Some(bounds) = bounds {
                    entity.insert(bounds);
                }
            } else if let Some(material) = fibre {
                entity.insert((MeshMaterial3d(material.clone()), crate::foliage::WindFibre));
                if let Some(bounds) = bounds {
                    entity.insert(bounds);
                }
            } else if let Some(material) = world
                .wet
                .as_ref()
                .filter(|_| damp)
                .and_then(|wet| wet.0.get(&batch.material))
            {
                entity.insert(MeshMaterial3d(material.clone()));
            } else {
                entity.insert(MeshMaterial3d(assets.materials[batch.material].clone()));
            }
            let permanently_shadowless = assets.templates.as_ref().is_some_and(|t| {
                t.glass_material(batch.material) || t.torch_material(batch.material)
            });
            let replaced_caster = !shadow_only && shadowed_materials.contains(&batch.material);
            if replaced_caster && !permanently_shadowless {
                entity.insert(super::shadows::ReplacedShadowCaster);
            }
            if replaced_caster || permanently_shadowless {
                entity.insert(bevy::light::NotShadowCaster);
            }
            handles.push(handle);
        }
        if first_upload {
            crate::transitions::enter(&mut commands, root, animation);
        }
        rendered.sections.insert(
            position,
            RenderedSection {
                micro_detail,
                appearance_epoch,
                entity: root,
                meshes: handles,
                mesh_epoch: epoch,
                detail_level: level,
            },
        );
    }
}

fn insert_rooted_wind(mesh: &mut Mesh, mut weights: Vec<[f32; 2]>) {
    weights.resize(mesh.count_vertices(), [0.0; 2]);
    if weights.iter().any(|weight| weight[1] < 0.0) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, weights);
        return;
    }
    let weights: Vec<[u16; 2]> = weights
        .into_iter()
        .map(|weight| {
            weight.map(|value| (value.clamp(0.0, 1.0) * f32::from(u16::MAX)).round() as u16)
        })
        .collect();
    // Interleaved vertex strides must remain four-byte aligned in every PBR pass.
    mesh.insert_attribute(
        bevy::mesh::MeshVertexAttribute {
            format: bevy::render::render_resource::VertexFormat::Unorm16x2,
            ..Mesh::ATTRIBUTE_UV_1
        },
        bevy::mesh::VertexAttributeValues::Unorm16x2(weights),
    );
}

fn remove_section(old: RenderedSection, commands: &mut Commands) {
    // Finish a live entrance before despawning, releasing its temporary materials
    // and draw-readiness entries as well as the owned meshes.
    crate::transitions::retire(
        commands,
        old.entity,
        old.meshes,
        terra_core::ChunkAnimation::None,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbor_revisions_keep_generation_hidden_but_placements_publish_immediately() {
        use super::super::{builder::MeshBatch, jobs::RenderJob};
        use bevy::tasks::AsyncComputeTaskPool;
        let mut config = terra_core::GameConfig::default();
        config.client.chunk_animation = terra_core::ChunkAnimation::Fade;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(config)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<BlockAssets>()
            .init_resource::<RenderedSections>()
            .init_resource::<RenderStatus>()
            .init_resource::<EditJournal>()
            .init_resource::<ChunkStore>()
            .init_resource::<RenderJobs>()
            .add_systems(Update, upload_sections);
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        app.world_mut()
            .resource_mut::<BlockAssets>()
            .materials
            .push(material);
        let position = terra_voxel::SectionPos::default();
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            position,
            terra_world::streaming::LoadedSection {
                data: terra_voxel::BlockSection::empty(position).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        // Saved edits in a newly streamed section still get their first fade.
        app.world_mut()
            .resource_mut::<EditJournal>()
            .record([0, 0, 0], 1)
            .unwrap();
        for revision in 0..6 {
            if revision > 0 {
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .record(
                        if revision < 4 { [16, 0, 0] } else { [0, 0, 0] },
                        if revision % 2 == 0 { 3 } else { 2 },
                    )
                    .unwrap();
            }
            if revision == 5 {
                // The first placement into a previously empty section is immediate too.
                let old = app
                    .world_mut()
                    .resource_mut::<RenderedSections>()
                    .sections
                    .remove(&position)
                    .unwrap();
                app.world_mut().despawn(old.entity);
                for mesh in old.meshes {
                    app.world_mut()
                        .resource_mut::<Assets<Mesh>>()
                        .remove(mesh.id());
                }
            }
            let epoch = app.world().resource::<EditJournal>().mesh_epoch(position);
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&position)
                .unwrap()
                .mesh_epoch = epoch;
            let mut batch = MeshBatch {
                positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                indices: vec![0, 1, 2],
                ..default()
            };
            if revision % 2 == 0 {
                batch.compact_indices();
            }
            app.world_mut().resource_mut::<RenderJobs>().0.insert(
                position,
                RenderJob {
                    ready: Some((Ok(vec![batch]), 0.0)),
                    micro_detail: true,
                    epoch,
                    appearance_epoch: 0,
                    level: super::super::DetailLevel::Near,
                    task: AsyncComputeTaskPool::get().spawn(std::future::pending()),
                },
            );
            app.update();
            let root = app.world().resource::<RenderedSections>().sections[&position].entity;
            assert_eq!(
                app.world()
                    .get::<crate::transitions::QueuedChunk>(root)
                    .is_some(),
                revision < 4
            );
            assert_eq!(
                app.world().get::<Visibility>(root),
                Some(if revision < 4 {
                    &Visibility::Hidden
                } else {
                    &Visibility::Inherited
                })
            );
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
            let rendered = &app.world().resource::<RenderedSections>().sections[&position];
            let mesh = app
                .world()
                .resource::<Assets<Mesh>>()
                .get(&rendered.meshes[0])
                .unwrap();
            assert_eq!(
                mesh.indices().unwrap().iter().collect::<Vec<_>>(),
                [0, 1, 2]
            );
            assert_eq!(
                matches!(mesh.indices(), Some(bevy::mesh::Indices::U16(_))),
                revision % 2 == 0
            );
            assert_eq!(
                app.world_mut()
                    .query::<&TerrainSection>()
                    .iter(app.world())
                    .count(),
                1
            );
        }
    }

    #[test]
    fn disabled_shadows_discard_completed_proxy_batches_before_upload_budgeting() {
        use super::super::{builder::MeshBatch, jobs::RenderJob};
        use bevy::tasks::AsyncComputeTaskPool;
        use terra_world::streaming::LoadedSection;

        for quality in [
            terra_core::ShadowQuality::Off,
            terra_core::ShadowQuality::High,
        ] {
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
                .init_resource::<crate::MeshUploadStatus>()
                .insert_resource(terra_core::GraphicsConfig {
                    shadow_quality: quality,
                    ..default()
                })
                .add_systems(
                    Update,
                    (upload_sections, super::super::shadows::release_disabled).chain(),
                );
            let material = app
                .world_mut()
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial::default());
            app.world_mut()
                .resource_mut::<BlockAssets>()
                .materials
                .push(material);
            let position = terra_voxel::SectionPos::default();
            app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
                position,
                LoadedSection {
                    data: terra_voxel::BlockSection::empty(position).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            let vertices = vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
            let batches = [false, true]
                .map(|shadow_only| MeshBatch {
                    shadow_only,
                    shadow_sources: if shadow_only { vec![0] } else { Vec::new() },
                    positions: vertices.clone(),
                    indices: vec![0, 1, 2],
                    ..default()
                })
                .into_iter()
                .collect();
            app.world_mut().resource_mut::<RenderJobs>().0.insert(
                position,
                RenderJob {
                    ready: Some((Ok(batches), 0.0)),
                    micro_detail: true,
                    epoch: 0,
                    appearance_epoch: 0,
                    level: super::super::DetailLevel::Near,
                    task: AsyncComputeTaskPool::get().spawn(std::future::pending()),
                },
            );
            app.update();
            let enabled = quality != terra_core::ShadowQuality::Off;
            let root = app.world().resource::<RenderedSections>().sections[&position].entity;
            assert_eq!(
                app.world().resource::<RenderedSections>().sections[&position]
                    .meshes
                    .len(),
                1 + usize::from(enabled)
            );
            assert_eq!(
                app.world().resource::<Assets<Mesh>>().len(),
                1 + usize::from(enabled)
            );
            assert_eq!(
                app.world()
                    .resource::<crate::MeshUploadStatus>()
                    .admitted_vertices,
                if enabled { 6 } else { 3 }
            );
            let mut colors = app
                .world_mut()
                .query_filtered::<(Entity, &Mesh3d), Without<super::super::shadows::ShadowProxy>>();
            let (color, mesh) = colors.single(app.world()).unwrap();
            let (color, mesh) = (color, mesh.0.clone());
            assert_eq!(
                app.world()
                    .get::<bevy::light::NotShadowCaster>(color)
                    .is_some(),
                enabled
            );
            assert_eq!(
                app.world()
                    .get::<super::super::shadows::ReplacedShadowCaster>(color)
                    .is_some(),
                enabled
            );
            assert_eq!(
                app.world()
                    .resource::<Assets<Mesh>>()
                    .get(&mesh)
                    .unwrap()
                    .attribute(Mesh::ATTRIBUTE_POSITION),
                Some(&bevy::mesh::VertexAttributeValues::Float32x3(vertices))
            );
            if enabled {
                app.world_mut()
                    .resource_mut::<terra_core::GraphicsConfig>()
                    .shadow_quality = terra_core::ShadowQuality::Off;
                app.update();
                assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
                assert_eq!(
                    app.world().resource::<RenderedSections>().sections[&position].entity,
                    root
                );
                assert_eq!(app.world().get::<Mesh3d>(color).unwrap().0, mesh);
                assert!(
                    app.world()
                        .get::<bevy::light::NotShadowCaster>(color)
                        .is_none()
                );
            }
        }
    }

    #[test]
    fn water_root_weights_keep_their_sign_in_the_gpu_vertex_layout() {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0; 3]; 3]);
        insert_rooted_wind(&mut mesh, vec![[0.0, 0.0], [0.0, -2.0], [0.0, -6.5]]);
        let Some(bevy::mesh::VertexAttributeValues::Float32x2(weights)) =
            mesh.attribute(Mesh::ATTRIBUTE_UV_1)
        else {
            panic!("Water bending must preserve signed weights");
        };
        assert_eq!(weights[2][1], -6.5);
    }
    use crate::blocks::{BlockTemplate, TemplateSet, cube_faces, definitions::BlockRenderKind};
    use crate::meshing::{
        DetailLevel,
        builder::{RenderOptions, compose_section_with_options},
    };
    use terra_voxel::{SectionPos, meshing::BlockFace};

    #[test]
    fn every_cap_vertex_layout_remains_aligned_for_forward_and_prepass_pipelines() {
        for id in 21..=26 {
            let mut templates = TemplateSet::default();
            templates.blocks.insert(
                id,
                BlockTemplate {
                    render_kind: BlockRenderKind::Cube,
                    face_materials: [0; 6],
                    faces: cube_faces(0.0, [0; 6], None),
                    side_cap: None,
                    bevel_width: 0.0,
                    surface: None,
                    model: Vec::new(),
                    details: Vec::new(),
                },
            );
            let underside = BlockFace {
                material: id,
                minimum: [2, 8, 2],
                width: 8,
                height: 8,
                u_axis: 0,
                v_axis: 2,
                side: 2,
                occupied_neighbor: false,
                edge_mask: 15,
            };
            for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
                let mut batches = compose_section_with_options(
                    &[
                        underside,
                        BlockFace {
                            side: 3,
                            ..underside
                        },
                    ],
                    &templates,
                    SectionPos::new(-1, 0, -1),
                    &RenderOptions {
                        water_level: None,
                        detail_level: level,
                        terrain: None,
                        exposure: None,
                        conditions: Default::default(),
                    },
                )
                .unwrap();
                assert_eq!(batches.len(), 1);
                let batch = batches.remove(0);
                assert_eq!(!batch.rooted_wind.is_empty(), level != DetailLevel::Far);
                let count = batch.positions.len();
                let rooted = !batch.rooted_wind.is_empty();
                let mut mesh = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::MAIN_WORLD,
                )
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, batch.positions)
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, batch.normals)
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, batch.uvs)
                .with_inserted_indices(bevy::mesh::Indices::U32(batch.indices));
                if !batch.colors.is_empty() {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, batch.colors);
                }
                if rooted {
                    insert_rooted_wind(&mut mesh, batch.rooted_wind);
                    let Some(bevy::mesh::VertexAttributeValues::Unorm16x2(weights)) =
                        mesh.attribute(Mesh::ATTRIBUTE_UV_1)
                    else {
                        panic!("Fibre wind weights must use the aligned format");
                    };
                    assert_eq!(weights.len(), count);
                    assert!(weights.iter().any(|weight| weight[1] > 0));
                    assert_eq!(
                        weights.last(),
                        Some(&[0, 0]),
                        "Static top faces must stay pinned"
                    );
                }
                assert_eq!(mesh.count_vertices(), count);
                let mut layouts = bevy::mesh::MeshVertexBufferLayouts::default();
                let layout = mesh.get_mesh_vertex_buffer_layout(&mut layouts);
                for prepass in [false, true] {
                    let mut attributes = vec![
                        Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
                        Mesh::ATTRIBUTE_NORMAL.at_shader_location(if prepass { 3 } else { 1 }),
                        Mesh::ATTRIBUTE_UV_0.at_shader_location(if prepass { 1 } else { 2 }),
                    ];
                    if rooted {
                        attributes.push(Mesh::ATTRIBUTE_UV_1.at_shader_location(if prepass {
                            2
                        } else {
                            3
                        }));
                    }
                    if mesh.contains_attribute(Mesh::ATTRIBUTE_COLOR) {
                        attributes.push(Mesh::ATTRIBUTE_COLOR.at_shader_location(if prepass {
                            7
                        } else {
                            5
                        }));
                    }
                    let buffer = layout.0.get_layout(&attributes).unwrap();
                    assert_eq!(
                        buffer.array_stride % 4,
                        0,
                        "Cap {id}, {level:?}, prepass={prepass}"
                    );
                    for attribute in buffer.attributes {
                        let alignment = attribute.format.size().min(4);
                        assert_eq!(attribute.offset % alignment, 0);
                    }
                }
            }
        }
    }
}
