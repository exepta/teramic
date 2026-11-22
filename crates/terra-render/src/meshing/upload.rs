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
    visibility: Option<Res<'w, super::visibility::TerrainVisibility>>,
    cache: Option<Res<'w, super::GeometryCache>>,
    far: Option<Res<'w, super::FarCache>>,
    materials: Option<Res<'w, Assets<StandardMaterial>>>,
    shadow_material: Option<Res<'w, super::shadows::OpaqueShadowMaterial>>,
    budget: Option<ResMut<'w, crate::MeshUploadStatus>>,
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    focus: Option<Res<'w, terra_world::streaming::ChunkFocus>>,
    config: Option<Res<'w, terra_core::GameConfig>>,
    graphics: Option<Res<'w, terra_core::GraphicsConfig>>,
    foliage: Option<Res<'w, crate::foliage::FoliageAssets>>,
    water: Option<Res<'w, crate::water::material::WaterAssets>>,
    wet: Option<Res<'w, crate::weather::surfaces::WetMaterials>>,
    depth: Option<Res<'w, crate::weather::surfaces::TerrainDepthMaterials>>,
    terrain: Option<Res<'w, terra_world::streaming::WorldTerrain>>,
    time: Option<Res<'w, Time>>,
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
    (mut status, mut motion): (
        ResMut<RenderStatus>,
        Option<ResMut<crate::liquid_motion::History>>,
    ),
) {
    let _scope = terra_core::profiling::ScopeTimer::new("terrain.upload");
    let visibility_changed = world.visibility.as_ref().is_some_and(|v| v.is_changed());
    if jobs.0.is_empty()
        && !world.chunks.is_changed()
        && !rendered.is_changed()
        && !visibility_changed
    {
        return;
    }
    let chunks = &world.chunks;
    if visibility_changed {
        let hidden: Vec<_> = rendered
            .sections
            .keys()
            .copied()
            .filter(|p| world.visibility.as_ref().is_some_and(|v| v.hidden(*p)))
            .collect();
        for p in hidden {
            if let Some(old) = rendered.sections.remove(&p) {
                crate::transitions::retire(
                    &mut commands,
                    old.entity,
                    old.meshes,
                    terra_core::ChunkAnimation::None,
                );
            }
        }
    }
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
    let fallback = world
        .config
        .is_none()
        .then(terra_core::ClientConfig::default);
    let client = world
        .config
        .as_ref()
        .map(|c| &c.client)
        .or(fallback.as_ref())
        .unwrap();
    let mut positions: Vec<_> = jobs.0.keys().copied().collect();
    positions.sort_by_key(|p| {
        (
            !super::jobs::interactive_edit(*p, &world.journal, &rendered),
            super::lod::distance_squared(*p, focus) as u64,
            p.y,
            p.z,
            p.x,
        )
    });
    for position in positions {
        if world
            .visibility
            .as_ref()
            .is_some_and(|v| v.hidden(position))
        {
            jobs.cancel(&position, Some(&mut meshes));
            continue;
        }
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
        // Streaming recapture temporarily changes the appearance epoch before
        // knowing whether any vertex payload changed. Keep finished/staged work
        // until that comparison settles, then reject only genuinely stale data.
        if world
            .cache
            .as_ref()
            .is_some_and(|cache| cache.exposure_pending(position))
        {
            continue;
        }
        if let Some((mut result, seconds)) = poll_job(job) {
            let previous = rendered.sections.get(&position);
            let stale_detail = world.focus.as_ref().is_some_and(|focus| {
                super::lod::select_configured_micro_detail(
                    position,
                    focus.0,
                    previous.map(|r| r.micro_detail),
                    client.blocks,
                ) != job.micro_detail
                    || super::lod::select_level(
                        position,
                        focus.0,
                        previous.map(|r| r.detail_level),
                        client,
                    ) != job.level
            });
            if stale_detail
                || job.epoch != world.journal.mesh_epoch(position)
                || job.appearance_epoch
                    != world.cache.as_ref().map_or_else(
                        || world.journal.appearance_epoch(position),
                        |cache| cache.appearance_epoch(position, &world.journal),
                    )
                || chunks
                    .loaded
                    .get(&position)
                    .is_none_or(|s| s.mesh_epoch != job.epoch)
            {
                if let Ok(batches) = &mut result {
                    crate::upload_budget::release_staged(batches, &mut meshes);
                }
                jobs.cancel(&position, Some(&mut meshes));
                jobs.1.push(position);
                status.completed_mesh_jobs += 1;
                status.composition_seconds += seconds;
                continue;
            }
            if previous.is_some_and(|old| {
                world.entering.contains(old.entity)
                    && old.mesh_epoch == job.epoch
                    && old.appearance_epoch == job.appearance_epoch
                    && old.detail_level == job.level
                    && old.micro_detail == job.micro_detail
            }) {
                if let Ok(batches) = &mut result {
                    crate::upload_budget::release_staged(batches, &mut meshes);
                }
                jobs.cancel(&position, Some(&mut meshes));
                status.completed_mesh_jobs += 1;
                status.composition_seconds += seconds;
                status.max_composition_seconds = status.max_composition_seconds.max(seconds);
                continue;
            }
            if !super::shadows::enabled(world.graphics.as_deref())
                && let Ok(batches) = &mut result
            {
                batches.retain_mut(|batch| {
                    if batch.shadow_only {
                        crate::upload_budget::release_staged(
                            std::slice::from_mut(batch),
                            &mut meshes,
                        );
                        false
                    } else {
                        true
                    }
                });
            }
            if world.budget.as_mut().is_some_and(|budget| {
                !budget.reserve_section(crate::upload_budget::UploadKind::Terrain)
            }) {
                job.ready = Some((result, seconds));
                break;
            }
            let mut deferred = false;
            if let (Ok(batches), Ok(origin)) = (&mut result, position.origin()) {
                let tint = world
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
                let column = tint
                    .as_ref()
                    .and_then(|_| world.chunks.prepared_column([position.x, position.z]));
                for batch in batches.iter_mut().filter(|batch| batch.staged.is_none()) {
                    if batch.positions.is_empty() || !batch.has_indices() {
                        continue;
                    }
                    if world.budget.as_mut().is_some_and(|budget| {
                        !budget.reserve_batch_bytes(
                            crate::upload_budget::UploadKind::Terrain,
                            batch.positions.len(),
                            crate::upload_budget::batch_bytes(batch),
                        )
                    }) {
                        deferred = true;
                        break;
                    }
                    prepare_batch(
                        batch,
                        &world,
                        &assets,
                        tint.as_ref().map(|t| (t, origin, column.as_deref())),
                        &mut meshes,
                        job.level == super::DetailLevel::Far,
                    );
                }
            }
            if deferred {
                // Keep prepared assets private until the complete replacement
                // can publish atomically; subsequent frames stage only the rest.
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
    for (position, epoch, appearance_epoch, level, micro_detail, mut result) in completed {
        jobs.0.remove(&position);
        if epoch != world.journal.mesh_epoch(position)
            || appearance_epoch
                != world.cache.as_ref().map_or_else(
                    || world.journal.appearance_epoch(position),
                    |cache| cache.appearance_epoch(position, &world.journal),
                )
            || chunks
                .loaded
                .get(&position)
                .is_none_or(|s| s.mesh_epoch != epoch)
        {
            if let Ok(batches) = &mut result {
                crate::upload_budget::release_staged(batches, &mut meshes);
            }
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
        batches.retain(|batch| batch.staged.is_some());
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
        if let Some(history) = motion.as_deref_mut() {
            let now = world.time.as_ref().map_or(0.0, |time| time.elapsed_secs());
            for kind in [
                crate::liquid_motion::Kind::Galle,
                crate::liquid_motion::Kind::Tar,
            ] {
                let present = batches.iter().any(|batch| {
                    assets.templates.as_ref().and_then(|templates| {
                        crate::liquid_motion::Kind::material(templates, batch.material)
                    }) == Some(kind)
                });
                if present {
                    continue;
                }
                // Curing changes the road's surface material in place; a wet
                // retirement overlay would flash on top of the dry coating.
                if kind == crate::liquid_motion::Kind::Tar
                    && assets.templates.as_ref().is_some_and(|templates| {
                        templates
                            .blocks
                            .get(&terra_core::roads::DRIED_TAR)
                            .is_some_and(|block| {
                                batches
                                    .iter()
                                    .any(|batch| batch.material == block.face_materials[3])
                            })
                    })
                {
                    history.remove(crate::liquid_motion::Key(position, kind));
                    continue;
                }
                if let Some((mesh, duration, slot)) =
                    history.drain(crate::liquid_motion::Key(position, kind), now)
                    && let Some(material) = world
                        .wet
                        .as_ref()
                        .and_then(|materials| materials.0.get(&slot))
                    && let Ok(origin) = position.origin()
                {
                    let handle = meshes.add(mesh);
                    commands.spawn((
                        Mesh3d(handle.clone()),
                        MeshMaterial3d(material.clone()),
                        Transform::from_translation(Vec3::from_array(origin.map(|v| v as f32))),
                        crate::liquid_motion::retiring(
                            crate::liquid_motion::Key(position, kind),
                            now + duration,
                            handle,
                        ),
                        crate::miniature::layers(),
                        bevy::light::NotShadowCaster,
                        Name::new("Draining viscous surface"),
                    ));
                }
            }
        }
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
        let shadowed_materials: std::collections::HashSet<_> = batches
            .iter()
            .filter(|batch| batch.shadow_only)
            .flat_map(|batch| batch.shadow_sources.iter().copied())
            .collect();
        let mut handles = Vec::with_capacity(batches.len());
        for mut batch in batches {
            let shadow_only = batch.shadow_only;
            let mut prepared = batch.staged.take().expect("complete upload is atomic");
            if let Some(mut mesh) = prepared.liquid.take()
                && let Some(history) = motion.as_deref_mut()
                && let Some(kind) = assets.templates.as_ref().and_then(|templates| {
                    crate::liquid_motion::Kind::material(templates, batch.material)
                })
            {
                let now = world.time.as_ref().map_or(0.0, |time| time.elapsed_secs());
                history.prepare(
                    crate::liquid_motion::Key(position, kind),
                    &mut mesh,
                    now,
                    !first_upload,
                    batch.material,
                );
                prepared.bounds = crate::liquid_motion::bounds(&mesh);
                if let Err(error) = meshes.insert(prepared.handle.id(), mesh) {
                    tracing::error!(%error,"Fluid interpolation mesh publication failed");
                }
                prepared.far = None;
            }
            let rooted_fibre = prepared.rooted;
            let damp = prepared.damp;
            let water = world.water.as_ref().filter(|_| {
                assets
                    .templates
                    .as_ref()
                    .is_some_and(|t| t.water_material(batch.material))
            });
            let coastal_quads = prepared.coastal_quads;
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
            let bounds = prepared.bounds;
            let handle = prepared.handle;
            let mut entity =
                commands.spawn((Mesh3d(handle.clone()), Transform::default(), ChildOf(root)));
            if let Some(bounds) = bounds {
                entity.insert((bounds, bevy::camera::visibility::NoAutoAabb));
            }
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
            if prepared.opaque_shadow
                && let Some(material) = world.shadow_material.as_ref()
            {
                entity.insert(MeshMaterial3d(material.0.clone()));
            } else if let Some(water) = water {
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
            } else if let Some(material) = world
                .depth
                .as_ref()
                .filter(|_| {
                    !rooted_fibre
                        && !damp
                        && !shadow_only
                        && crate::weather::surfaces::dry_terrain_uses_shared_surface_material()
                })
                .and_then(|depth| depth.0.get(&batch.material))
            {
                entity.insert(MeshMaterial3d(material.clone()));
            } else {
                entity.insert(MeshMaterial3d(assets.materials[batch.material].clone()));
            }
            let permanently_shadowless = assets.templates.as_ref().is_some_and(|t| {
                t.glass_material(batch.material)
                    || t.torch_material(batch.material)
                    || t.fog_material(batch.material)
                    || t.bile_material(batch.material)
            });
            let replaced_caster = !shadow_only && shadowed_materials.contains(&batch.material);
            if replaced_caster && !permanently_shadowless {
                entity.insert(super::shadows::ReplacedShadowCaster);
            }
            if replaced_caster || permanently_shadowless {
                entity.insert(bevy::light::NotShadowCaster);
            }
            if let Some(geometry) = prepared.far.take() {
                let material = world
                    .wet
                    .as_ref()
                    .filter(|_| damp)
                    .and_then(|wet| wet.0.get(&batch.material))
                    .map(|m| super::far_batches::Material::Wet(m.clone()))
                    .or_else(|| {
                        world
                            .depth
                            .as_ref()
                            .filter(|_| {
                                !rooted_fibre && !damp && !shadow_only
                                    && crate::weather::surfaces::dry_terrain_uses_shared_surface_material()
                            })
                            .and_then(|depth| depth.0.get(&batch.material))
                            .map(|m| super::far_batches::Material::Depth(m.clone()))
                    })
                    .unwrap_or_else(|| {
                        super::far_batches::Material::Standard(
                            assets.materials[batch.material].clone(),
                        )
                    });
                entity.insert(super::far_batches::Source {
                    geometry,
                    position,
                    root,
                    material,
                    shadowless: replaced_caster || permanently_shadowless,
                });
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

fn prepare_batch(
    batch: &mut super::builder::MeshBatch,
    world: &RenderWorld,
    assets: &BlockAssets,
    tint: Option<(
        &crate::water::biome::WaterTint,
        [i64; 3],
        Option<&terra_world::PreparedColumn>,
    )>,
    meshes: &mut Assets<Mesh>,
    far: bool,
) {
    let opaque_shadow =
        batch.shadow_only && batch.normals.is_empty() && batch.packed_normals.is_empty();
    let rooted = !batch.rooted_wind.is_empty();
    let damp = !rooted && !batch.shadow_only && !batch.surface_water.is_empty();
    let water = world.water.is_some()
        && assets
            .templates
            .as_ref()
            .is_some_and(|t| t.water_material(batch.material));
    let coastal_quads = if water {
        batch
            .colors
            .chunks_exact(4)
            .filter(|q| (q[0][3] - 6.0).abs() < 0.001)
            .count()
    } else {
        0
    };
    let tints = if water {
        tint.map(|(tint, origin, column)| {
            batch
                .positions
                .iter()
                .map(|&point| tint.optics(origin, point, column))
                .unzip::<_, _, Vec<_>, Vec<_>>()
        })
    } else {
        None
    };
    batch.finish_tangents();
    let indices = batch.take_indices();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        std::mem::take(&mut batch.positions),
    )
    .with_inserted_indices(indices);
    if !batch.bile_motion.is_empty() {
        mesh.insert_attribute(
            crate::liquid_motion::POUR_COORDS,
            std::mem::take(&mut batch.bile_motion),
        );
    }
    if !batch.uvs.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, std::mem::take(&mut batch.uvs));
    }
    if !batch.packed_tangents.is_empty() {
        mesh.insert_attribute(
            bevy::mesh::MeshVertexAttribute {
                format: bevy::render::render_resource::VertexFormat::Snorm8x4,
                ..Mesh::ATTRIBUTE_TANGENT
            },
            bevy::mesh::VertexAttributeValues::Snorm8x4(std::mem::take(&mut batch.packed_tangents)),
        );
    } else if !batch.tangents.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, std::mem::take(&mut batch.tangents));
    }
    if rooted {
        insert_rooted_wind(&mut mesh, std::mem::take(&mut batch.rooted_wind));
    } else if damp && !batch.surface_water.is_empty() {
        let mut values = std::mem::take(&mut batch.surface_water);
        values.resize(mesh.count_vertices(), [0.; 2]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, values);
    }
    if let Some((tints, blood)) = tints {
        mesh.insert_attribute(crate::water::biome::ATTRIBUTE_BIOME_TINT, tints);
        mesh.insert_attribute(crate::water::biome::ATTRIBUTE_BLOOD_WEIGHT, blood);
    }
    if !batch.packed_normals.is_empty() {
        mesh.insert_attribute(
            bevy::mesh::MeshVertexAttribute {
                format: bevy::render::render_resource::VertexFormat::Snorm8x4,
                ..Mesh::ATTRIBUTE_NORMAL
            },
            bevy::mesh::VertexAttributeValues::Snorm8x4(std::mem::take(&mut batch.packed_normals)),
        );
    } else if !batch.normals.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, std::mem::take(&mut batch.normals));
    }
    if !batch.packed_colors.is_empty() {
        mesh.insert_attribute(
            bevy::mesh::MeshVertexAttribute {
                format: bevy::render::render_resource::VertexFormat::Unorm8x4,
                ..Mesh::ATTRIBUTE_COLOR
            },
            bevy::mesh::VertexAttributeValues::Unorm8x4(std::mem::take(&mut batch.packed_colors)),
        );
    } else if let Some(colors) = batch.half_colors.take() {
        mesh.insert_attribute(
            bevy::mesh::MeshVertexAttribute {
                format: bevy::render::render_resource::VertexFormat::from(&colors),
                ..Mesh::ATTRIBUTE_COLOR
            },
            colors,
        );
    } else if !batch.colors.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, std::mem::take(&mut batch.colors));
    }
    let wind = world.foliage.as_ref().is_some_and(|a| {
        a.materials.contains_key(&batch.material)
            || a.flames.contains_key(&batch.material)
            || a.reference.contains_key(&batch.material)
            || rooted && a.fibres.contains_key(&batch.material)
    });
    if !batch.shadow_only
        && !rooted
        && !wind
        && !water
        && assets
            .templates
            .as_ref()
            .is_some_and(|templates| templates.authored_terrain_material(batch.material))
    {
        compress_static_frame(&mut mesh);
    }
    // Section meshes are immutable until replaced. Keep shader displacement
    // padding and exclude them from the per-frame asset-change bounds scan.
    let bounds = mesh.get_aabb().map(|mut bounds| {
        if wind {
            bounds.half_extents += Vec3A::splat(crate::foliage::WIND_BOUNDS);
        }
        bounds
    });
    let far = if world.far.is_some()
        && far
        && !batch.shadow_only
        && !wind
        && !water
        && world
            .materials
            .as_ref()
            .and_then(|materials| materials.get(&assets.materials[batch.material]))
            .is_some_and(|m| m.alpha_mode == AlphaMode::Opaque)
    {
        world.far.as_ref().and_then(|cache| cache.capture(&mesh))
    } else {
        None
    };
    batch.staged = Some(super::staging::StagedMesh {
        opaque_shadow,
        liquid: assets
            .templates
            .as_ref()
            .and_then(|templates| crate::liquid_motion::Kind::material(templates, batch.material))
            .map(|_| mesh.clone()),
        handle: meshes.add(mesh),
        bounds,
        rooted,
        damp,
        coastal_quads,
        far,
    });
}

fn compress_static_frame(mesh: &mut Mesh) {
    use bevy::mesh::VertexAttributeValues;

    if let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        && !normals.is_empty()
        && normals.iter().all(|normal| {
            normal.iter().all(|value| value.is_finite())
                && (Vec3::from_array(*normal).length_squared() - 1.0).abs() <= 0.001
        })
    {
        // Static authored materials use stock forward/prepass decompression.
        let _ = mesh.compress_normals();
    }
    let Some(VertexAttributeValues::Float32x4(tangents)) = mesh.attribute(Mesh::ATTRIBUTE_TANGENT)
    else {
        return;
    };
    if tangents.is_empty()
        || tangents.iter().any(|tangent| {
            !tangent.iter().all(|value| value.is_finite())
                || (Vec3::new(tangent[0], tangent[1], tangent[2]).length_squared() - 1.0).abs()
                    > 0.001
                || (tangent[3].abs() - 1.0).abs() > 0.001
        })
    {
        return;
    }
    // Native octahedral packing keeps handedness; normals/tangents use 4 B each.
    let _ = mesh.compress_tangents();
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

    fn worker_batches(
        batches: Vec<super::super::builder::MeshBatch>,
    ) -> Vec<super::super::builder::MeshBatch> {
        let mut batches = crate::upload_budget::split_batches(batches).unwrap();
        for batch in &mut batches {
            batch.compact_indices();
        }
        batches
    }

    #[test]
    fn immutable_bounds_keep_displacement_padding_during_asset_updates() {
        use bevy::camera::{
            primitives::Aabb,
            visibility::{NoAutoAabb, calculate_bounds},
        };
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Mesh>()
            .add_systems(Last, calculate_bounds);
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(Cuboid::default()));
        let padded = Aabb {
            center: Vec3A::ZERO,
            half_extents: Vec3A::splat(0.5 + crate::foliage::WIND_BOUNDS),
        };
        let fixed = app
            .world_mut()
            .spawn((Mesh3d(mesh.clone()), padded, NoAutoAabb))
            .id();
        let dynamic = app.world_mut().spawn(Mesh3d(mesh.clone())).id();
        app.update();
        for size in [2., 4., 1.] {
            *app.world_mut()
                .resource_mut::<Assets<Mesh>>()
                .get_mut(&mesh)
                .unwrap() = Mesh::from(Cuboid::from_length(size));
            app.update();
            assert_eq!(
                app.world().get::<Aabb>(fixed).unwrap().half_extents,
                padded.half_extents
            );
            assert_eq!(
                app.world().get::<Aabb>(dynamic).unwrap().half_extents,
                Vec3A::splat(size * 0.5)
            );
        }
    }

    #[test]
    fn worker_prepared_half_tints_are_moved_unchanged_into_the_render_mesh() {
        use bevy::{ecs::system::RunSystemOnce, mesh::VertexAttributeValues};
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<BlockAssets>()
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>();
        app.world_mut()
            .run_system_once(
                |world: RenderWorld, assets: Res<BlockAssets>, mut meshes: ResMut<Assets<Mesh>>| {
                    let mut batch = super::super::builder::MeshBatch {
                        positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
                        normals: vec![[0.0, 1.0, 0.0]; 3],
                        colors: vec![[1.25, -0.125, 2.5, 6.0]; 3],
                        indices: vec![0, 1, 2],
                        ..default()
                    };
                    super::super::precision::compact_batch_tints(&mut batch);
                    let original = batch.half_colors.as_ref().unwrap().clone();
                    let allocation = batch.half_colors.as_ref().unwrap().get_bytes().as_ptr();
                    batch.compact_indices();
                    prepare_batch(&mut batch, &world, &assets, None, &mut meshes, false);
                    let mesh = meshes.get(&batch.staged.as_ref().unwrap().handle).unwrap();
                    let colors = mesh.attribute(Mesh::ATTRIBUTE_COLOR).unwrap();
                    assert!(matches!(colors, VertexAttributeValues::Float16x4(_)));
                    assert_eq!(colors, &original);
                    assert_eq!(colors.get_bytes().as_ptr(), allocation);
                    assert!(batch.half_colors.is_none());
                    assert!(batch.colors.is_empty());
                },
            )
            .unwrap();
    }

    #[test]
    fn staging_uploads_authored_tangents_without_regenerating_them() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<BlockAssets>()
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>();
        app.world_mut()
            .run_system_once(
                |world: RenderWorld, assets: Res<BlockAssets>, mut meshes: ResMut<Assets<Mesh>>| {
                    for packed in [false, true] {
                        let tangents = vec![[0.0, 0.0, 1.0, -1.0]; 3];
                        let mut batch = super::super::builder::MeshBatch {
                            positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
                            normals: vec![[0.0, 1.0, 0.0]; 3],
                            tangents: tangents.clone(),
                            uvs: vec![[0.0; 2]; 3],
                            indices: vec![0, 1, 2],
                            ..default()
                        };
                        if packed {
                            crate::foliage::packing::pack(&mut batch);
                        }
                        batch.compact_indices();
                        prepare_batch(&mut batch, &world, &assets, None, &mut meshes, false);
                        let mesh = meshes.get(&batch.staged.as_ref().unwrap().handle).unwrap();
                        assert_eq!(
                            mesh.attribute(Mesh::ATTRIBUTE_TANGENT),
                            Some(&bevy::mesh::VertexAttributeValues::Float32x4(tangents))
                        );
                        assert_eq!(mesh.count_vertices(), 3);
                        assert!(batch.tangents.is_empty());
                    }
                },
            )
            .unwrap();
    }

    #[test]
    fn native_frame_compression_preserves_frames_and_other_attributes() {
        use bevy::mesh::{
            MeshAttributeCompressionFlags, MeshVertexBufferLayouts, VertexAttributeValues,
            octahedral_decode_signed, octahedral_decode_tangent,
        };
        let directions = [
            Vec3::X,
            Vec3::NEG_X,
            Vec3::Y,
            Vec3::NEG_Y,
            Vec3::Z,
            Vec3::NEG_Z,
            Vec3::new(0.23, -0.77, 0.44).normalize(),
            Vec3::new(-0.49, 0.31, -0.82).normalize(),
        ];
        let tangents: Vec<_> = directions
            .into_iter()
            .flat_map(|direction| [-1., 1.].map(|sign| direction.extend(sign).to_array()))
            .collect();
        let normals: Vec<_> = directions
            .into_iter()
            .flat_map(|direction| [direction.to_array(); 2])
            .collect();
        let count = tangents.len();
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[1., 2., 3.]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.1, 0.9]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, vec![[0.3, -6.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.2, 0.4, 0.7, 1.]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, tangents.clone());
        let original = mesh.clone();
        compress_static_frame(&mut mesh);
        assert_eq!(
            original.get_vertex_buffer_size() - mesh.get_vertex_buffer_size(),
            count * 20
        );
        for attribute in [
            Mesh::ATTRIBUTE_POSITION,
            Mesh::ATTRIBUTE_UV_0,
            Mesh::ATTRIBUTE_UV_1,
            Mesh::ATTRIBUTE_COLOR,
        ] {
            assert_eq!(mesh.attribute(attribute), original.attribute(attribute));
        }
        assert_eq!(mesh.get_aabb(), original.get_aabb());
        let Some(VertexAttributeValues::Snorm16x2(encoded)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("native normal attribute was not compressed");
        };
        for (encoded, original) in encoded.iter().zip(normals) {
            let encoded = Vec2::from_array(encoded.map(|v| f32::from(v) / 32767.));
            let direction = octahedral_decode_signed(encoded);
            assert!(direction.distance(Vec3::from_array(original)) < 0.0002);
        }
        let Some(VertexAttributeValues::Snorm16x2(encoded)) =
            mesh.attribute(Mesh::ATTRIBUTE_TANGENT)
        else {
            panic!("native tangent attribute was not compressed");
        };
        for (encoded, original) in encoded.iter().zip(tangents) {
            let encoded = Vec2::from_array(encoded.map(|v| f32::from(v) / 32767.));
            let (direction, handedness) = octahedral_decode_tangent(encoded);
            let source = Vec3::new(original[0], original[1], original[2]);
            assert!(direction.distance(source) < 0.0002);
            assert_eq!(handedness, original[3]);
        }
        let layout = mesh.get_mesh_vertex_buffer_layout(&mut MeshVertexBufferLayouts::default());
        assert!(layout.0.get_attribute_compression().contains(
            MeshAttributeCompressionFlags::COMPRESS_TANGENT
                | MeshAttributeCompressionFlags::COMPRESS_NORMAL,
        ));
    }

    #[test]
    fn tangent_compression_preserves_invalid_and_nonstandard_float_frames() {
        use bevy::mesh::VertexAttributeValues;
        for tangent in [
            [0., 0., 0., 1.],
            [2., 0., 0., 1.],
            [1., 0., 0., 0.],
            [1., 0., 0., 0.5],
            [f32::NAN, 0., 0., 1.],
            [f32::INFINITY, 0., 0., 1.],
            [1., 0., 0., f32::NAN],
        ] {
            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 1., 0.]])
            .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, vec![tangent]);
            compress_static_frame(&mut mesh);
            assert!(matches!(
                mesh.attribute(Mesh::ATTRIBUTE_NORMAL),
                Some(VertexAttributeValues::Snorm16x2(_))
            ));
            let Some(VertexAttributeValues::Float32x4(actual)) =
                mesh.attribute(Mesh::ATTRIBUTE_TANGENT)
            else {
                panic!("unsupported tangent frames must retain the original format");
            };
            assert_eq!(actual[0].map(f32::to_bits), tangent.map(f32::to_bits));
        }
    }

    #[test]
    fn normal_compression_preserves_invalid_and_already_packed_normals() {
        use bevy::mesh::VertexAttributeValues;
        for normal in [
            [0., 0., 0.],
            [2., 0., 0.],
            [f32::NAN, 0., 0.],
            [0., f32::INFINITY, 0.],
        ] {
            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![normal])
            .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, vec![[1., 0., 0., -1.]]);
            compress_static_frame(&mut mesh);
            let Some(VertexAttributeValues::Float32x3(actual)) =
                mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
            else {
                panic!("unsupported normals must retain the original format");
            };
            assert_eq!(actual[0].map(f32::to_bits), normal.map(f32::to_bits));
            assert!(matches!(
                mesh.attribute(Mesh::ATTRIBUTE_TANGENT),
                Some(VertexAttributeValues::Snorm16x2(_))
            ));
        }
        let packed = VertexAttributeValues::Snorm8x4(vec![[0, 127, 0, 0]]);
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(
            bevy::mesh::MeshVertexAttribute {
                format: bevy::render::render_resource::VertexFormat::Snorm8x4,
                ..Mesh::ATTRIBUTE_NORMAL
            },
            packed.clone(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, vec![[1., 0., 0., -1.]]);
        compress_static_frame(&mut mesh);
        assert_eq!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL), Some(&packed));
        assert!(matches!(
            mesh.attribute(Mesh::ATTRIBUTE_TANGENT),
            Some(VertexAttributeValues::Snorm16x2(_))
        ));
    }

    #[test]
    fn staging_compresses_only_static_authored_terrain_frames() {
        use bevy::{ecs::system::RunSystemOnce, mesh::VertexAttributeValues};
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<BlockAssets>()
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .init_resource::<crate::foliage::FoliageAssets>();
        app.world_mut().resource_mut::<BlockAssets>().templates = Some(std::sync::Arc::new(
            crate::blocks::authored_terrain_templates(),
        ));
        app.world_mut()
            .resource_mut::<crate::foliage::FoliageAssets>()
            .materials
            .insert(1, Handle::default());
        app.world_mut()
            .run_system_once(
                |world: RenderWorld, assets: Res<BlockAssets>, mut meshes: ResMut<Assets<Mesh>>| {
                    for material in [0, 1, 2, 7] {
                        let mut batch = super::super::builder::MeshBatch {
                            material,
                            positions: vec![[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]],
                            normals: vec![[0., 1., 0.]; 3],
                            tangents: vec![[0., 0., 1., -1.]; 3],
                            uvs: vec![[0., 0.]; 3],
                            indices: vec![0, 1, 2],
                            ..default()
                        };
                        prepare_batch(&mut batch, &world, &assets, None, &mut meshes, false);
                        let mesh = meshes.get(&batch.staged.unwrap().handle).unwrap();
                        if material == 0 || material == 2 {
                            assert!(matches!(
                                mesh.attribute(Mesh::ATTRIBUTE_TANGENT),
                                Some(VertexAttributeValues::Snorm16x2(_))
                            ));
                            assert!(matches!(
                                mesh.attribute(Mesh::ATTRIBUTE_NORMAL),
                                Some(VertexAttributeValues::Snorm16x2(_))
                            ));
                        } else {
                            assert!(matches!(
                                mesh.attribute(Mesh::ATTRIBUTE_TANGENT),
                                Some(VertexAttributeValues::Float32x4(_))
                            ));
                            assert!(matches!(
                                mesh.attribute(Mesh::ATTRIBUTE_NORMAL),
                                Some(VertexAttributeValues::Float32x3(_))
                            ));
                        }
                        assert_eq!(mesh.count_vertices(), 3);
                    }
                },
            )
            .unwrap();
    }

    #[test]
    fn worker_packed_frames_stage_without_octahedral_decoding_or_bounds_changes() {
        use bevy::{
            ecs::system::RunSystemOnce,
            mesh::{MeshAttributeCompressionFlags, MeshVertexBufferLayouts, VertexAttributeValues},
        };
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<BlockAssets>()
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>();
        app.world_mut().resource_mut::<BlockAssets>().templates = Some(std::sync::Arc::new(
            crate::blocks::authored_terrain_templates(),
        ));
        app.world_mut()
            .run_system_once(
                |world: RenderWorld, assets: Res<BlockAssets>, mut meshes: ResMut<Assets<Mesh>>| {
                    for material in [0, 2, 7] {
                        let mut batch = super::super::builder::MeshBatch {
                            material,
                            positions: vec![[-4., -2., -8.], [6., 4., 2.], [0.; 3]],
                            normals: vec![[0., 1., 0.]; 3],
                            tangents: vec![[0., 0., 1., -1.]; 3],
                            uvs: vec![[0.25, 0.75]; 3],
                            indices: vec![0, 1, 2],
                            ..default()
                        };
                        batch.pack_static_frame();
                        prepare_batch(&mut batch, &world, &assets, None, &mut meshes, false);
                        let staged = batch.staged.unwrap();
                        let mesh = meshes.get(&staged.handle).unwrap();
                        assert_eq!(
                            mesh.attribute(Mesh::ATTRIBUTE_NORMAL),
                            Some(&VertexAttributeValues::Snorm8x4(vec![[0, 127, 0, 0]; 3]))
                        );
                        assert_eq!(
                            mesh.attribute(Mesh::ATTRIBUTE_TANGENT),
                            Some(&VertexAttributeValues::Snorm8x4(vec![[0, 0, 127, -127]; 3]))
                        );
                        assert_eq!(mesh.count_vertices(), 3);
                        assert_eq!(
                            mesh.attribute(Mesh::ATTRIBUTE_UV_0),
                            Some(&VertexAttributeValues::Float32x2(vec![[0.25, 0.75]; 3]))
                        );
                        let bounds = staged.bounds.unwrap();
                        assert_eq!(bounds.center, Vec3A::new(1., 1., -3.));
                        assert_eq!(bounds.half_extents, Vec3A::new(5., 3., 5.));
                        let layout = mesh
                            .get_mesh_vertex_buffer_layout(&mut MeshVertexBufferLayouts::default());
                        assert!(!layout.0.get_attribute_compression().intersects(
                            MeshAttributeCompressionFlags::COMPRESS_NORMAL
                                | MeshAttributeCompressionFlags::COMPRESS_TANGENT
                        ));
                    }
                },
            )
            .unwrap();
    }

    #[test]
    fn staged_bounds_cover_static_and_wind_displaced_geometry() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<BlockAssets>()
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .init_resource::<crate::foliage::FoliageAssets>();
        app.world_mut()
            .resource_mut::<crate::foliage::FoliageAssets>()
            .materials
            .insert(1, Handle::default());
        app.world_mut()
            .run_system_once(
                |world: RenderWorld, assets: Res<BlockAssets>, mut meshes: ResMut<Assets<Mesh>>| {
                    for material in [0, 1] {
                        let mut batch = super::super::builder::MeshBatch {
                            material,
                            positions: vec![[-4., -2., -8.], [6., 4., 2.], [0.; 3]],
                            indices: vec![0, 1, 2],
                            ..default()
                        };
                        prepare_batch(&mut batch, &world, &assets, None, &mut meshes, false);
                        let bounds = batch.staged.unwrap().bounds.unwrap();
                        assert_eq!(bounds.center, Vec3A::new(1., 1., -3.));
                        let padding = if material == 1 {
                            crate::foliage::WIND_BOUNDS
                        } else {
                            0.
                        };
                        assert_eq!(
                            bounds.half_extents,
                            Vec3A::new(5., 3., 5.) + Vec3A::splat(padding)
                        );
                    }
                },
            )
            .unwrap();
    }

    #[test]
    fn staging_preserves_shadow_kind_after_draining_vertex_attributes() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<BlockAssets>()
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>();
        app.world_mut()
            .run_system_once(
                |world: RenderWorld, assets: Res<BlockAssets>, mut meshes: ResMut<Assets<Mesh>>| {
                    for (shadow, normals, packed, expected) in [
                        (true, false, false, true),
                        (true, true, false, false),
                        (true, false, true, false),
                        (false, false, false, false),
                    ] {
                        let mut batch = super::super::builder::MeshBatch {
                            shadow_only: shadow,
                            positions: vec![[0.; 3]; 3],
                            indices: vec![0, 1, 2],
                            normals: if normals {
                                vec![[0., 1., 0.]; 3]
                            } else {
                                vec![]
                            },
                            packed_normals: if packed {
                                vec![[0, 127, 0, 0]; 3]
                            } else {
                                vec![]
                            },
                            ..default()
                        };
                        prepare_batch(&mut batch, &world, &assets, None, &mut meshes, false);
                        assert!(batch.normals.is_empty());
                        assert!(batch.packed_normals.is_empty());
                        assert_eq!(batch.staged.unwrap().opaque_shadow, expected);
                    }
                },
            )
            .unwrap();
    }

    #[test]
    fn multi_material_section_stages_over_frames_and_swaps_atomically() {
        use super::super::{builder::MeshBatch, jobs::RenderJob};
        for (vertices, count) in [
            (crate::upload_budget::TERRAIN_VERTICES / 2, 5),
            (crate::upload_budget::TERRAIN_VERTICES * 2, 2),
            (3, 12),
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
                    props: Vec::new(),
                },
            );
            let old = app.world_mut().spawn_empty().id();
            app.world_mut()
                .resource_mut::<RenderedSections>()
                .sections
                .insert(
                    position,
                    RenderedSection {
                        micro_detail: false,
                        appearance_epoch: 0,
                        entity: old,
                        meshes: Vec::new(),
                        mesh_epoch: 0,
                        detail_level: super::super::DetailLevel::Near,
                    },
                );
            let batch = || MeshBatch {
                positions: vec![[0.; 3]; vertices],
                indices: (0..(vertices - vertices % 3) as u32).collect(),
                ..default()
            };
            let batches = worker_batches(std::iter::repeat_with(batch).take(count).collect());
            let expected_meshes = batches.len();
            let expected_vertices: usize = batches.iter().map(|batch| batch.positions.len()).sum();
            let task =
                bevy::tasks::AsyncComputeTaskPool::get().spawn(async { (Ok(Vec::new()), 0.) });
            app.world_mut().resource_mut::<RenderJobs>().0.insert(
                position,
                RenderJob {
                    ready: Some((Ok(batches), 0.)),
                    micro_detail: false,
                    epoch: 0,
                    appearance_epoch: 0,
                    level: super::super::DetailLevel::Near,
                    task,
                },
            );
            assert!(
                app.world_mut()
                    .resource_mut::<crate::MeshUploadStatus>()
                    .reserve(
                        crate::upload_budget::UploadKind::Terrain,
                        crate::upload_budget::TERRAIN_VERTICES,
                    )
            );
            app.update();
            assert_eq!(
                app.world()
                    .resource::<crate::MeshUploadStatus>()
                    .admitted_vertices,
                crate::upload_budget::TERRAIN_VERTICES
            );
            assert_eq!(
                app.world().resource::<RenderedSections>().sections[&position].entity,
                old
            );
            assert!(app.world().get_entity(old).is_ok());
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
            let mut staged_vertices = 0;
            let mut finished = false;
            let mut previous_assets = 0;
            for _ in 0..64 {
                app.world_mut()
                    .insert_resource(crate::MeshUploadStatus::default());
                app.update();
                let admitted = app
                    .world()
                    .resource::<crate::MeshUploadStatus>()
                    .admitted_vertices;
                assert!(admitted <= crate::upload_budget::TERRAIN_VERTICES);
                assert!(
                    app.world()
                        .resource::<crate::MeshUploadStatus>()
                        .admitted_bytes
                        <= crate::upload_budget::TERRAIN_BYTES
                );
                staged_vertices += admitted;
                let staged_count = app
                    .world()
                    .resource::<RenderJobs>()
                    .0
                    .get(&position)
                    .and_then(|job| job.ready.as_ref())
                    .and_then(|(result, _)| result.as_ref().ok())
                    .map_or(expected_meshes, |batches| {
                        batches
                            .iter()
                            .filter(|batch| batch.staged.is_some())
                            .count()
                    });
                assert_eq!(
                    app.world().resource::<Assets<Mesh>>().len(),
                    staged_count,
                    "previously staged material batches must not allocate again"
                );
                assert!(staged_count >= previous_assets);
                previous_assets = staged_count;
                if app
                    .world()
                    .resource::<RenderJobs>()
                    .0
                    .contains_key(&position)
                {
                    assert_eq!(
                        app.world().resource::<RenderedSections>().sections[&position].entity,
                        old
                    );
                    assert!(app.world().get_entity(old).is_ok());
                    assert_eq!(app.world().resource::<RenderJobs>().0.len(), 1);
                    assert_eq!(
                        app.world().resource::<RenderStatus>().completed_mesh_jobs,
                        0
                    );
                } else {
                    finished = true;
                    break;
                }
            }
            assert!(
                finished,
                "split worker results must finish within a bounded frame count"
            );
            assert_eq!(staged_vertices, expected_vertices);
            let root = app.world().resource::<RenderedSections>().sections[&position].entity;
            assert_ne!(root, old);
            assert!(app.world().get_entity(old).is_err());
            for child in app.world().get::<Children>(root).unwrap().iter() {
                assert!(
                    app.world()
                        .get::<bevy::camera::primitives::Aabb>(child)
                        .is_some()
                );
                assert!(
                    app.world()
                        .get::<bevy::camera::visibility::NoAutoAabb>(child)
                        .is_some()
                );
                assert!(
                    app.world()
                        .get::<bevy::camera::visibility::NoFrustumCulling>(child)
                        .is_none()
                );
            }
            assert_eq!(
                app.world().resource::<RenderedSections>().sections[&position]
                    .meshes
                    .len(),
                expected_meshes
            );
            assert!(app.world().resource::<RenderJobs>().0.is_empty());
            assert_eq!(
                app.world().resource::<RenderStatus>().completed_mesh_jobs,
                1
            );
        }
    }

    fn entering_upload_fixture() -> (App, terra_voxel::SectionPos, Entity) {
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
            .init_resource::<crate::transitions::LodSwapStatus>()
            .add_systems(Update, upload_sections);
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        app.world_mut()
            .resource_mut::<BlockAssets>()
            .materials
            .push(material.clone());
        let position = terra_voxel::SectionPos::default();
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            position,
            terra_world::streaming::LoadedSection {
                data: terra_voxel::BlockSection::empty(position).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let root = app
            .world_mut()
            .spawn((Transform::default(), Visibility::Inherited))
            .id();
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(Cuboid::default()));
        app.world_mut().spawn((
            ChildOf(root),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
        ));
        crate::transitions::start_test_fade(app.world_mut(), root);
        app.world_mut()
            .resource_mut::<RenderedSections>()
            .sections
            .insert(
                position,
                RenderedSection {
                    micro_detail: true,
                    appearance_epoch: 0,
                    entity: root,
                    meshes: vec![mesh],
                    mesh_epoch: 0,
                    detail_level: super::super::DetailLevel::Near,
                },
            );
        (app, position, root)
    }

    fn completed_upload_job(
        level: super::super::DetailLevel,
        micro_detail: bool,
    ) -> super::super::jobs::RenderJob {
        super::super::jobs::RenderJob {
            ready: Some((
                Ok(vec![super::super::builder::MeshBatch {
                    positions: vec![[0.; 3], [1., 0., 0.], [0., 1., 0.]],
                    indices: vec![0, 1, 2],
                    ..default()
                }]),
                0.025,
            )),
            micro_detail,
            epoch: 0,
            appearance_epoch: 0,
            level,
            task: bevy::tasks::AsyncComputeTaskPool::get().spawn(std::future::pending()),
        }
    }

    #[test]
    fn identical_entering_results_release_jobs_without_meshes_or_upload_budget() {
        for exhausted_budget in [false, true] {
            let (mut app, position, root) = entering_upload_fixture();
            let reserved = if exhausted_budget {
                assert!(
                    app.world_mut()
                        .resource_mut::<crate::MeshUploadStatus>()
                        .reserve(
                            crate::upload_budget::UploadKind::Terrain,
                            crate::upload_budget::TERRAIN_VERTICES,
                        )
                );
                crate::upload_budget::TERRAIN_VERTICES
            } else {
                0
            };
            for completed in 1..=8 {
                app.world_mut().resource_mut::<RenderJobs>().0.insert(
                    position,
                    completed_upload_job(super::super::DetailLevel::Near, true),
                );
                app.update();
                assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
                let budget = app.world().resource::<crate::MeshUploadStatus>();
                assert_eq!(budget.admitted_vertices, reserved);
                assert_eq!(budget.deferred_batches, 0);
                let jobs = app.world().resource::<RenderJobs>();
                assert!(jobs.0.is_empty());
                assert!(jobs.1.is_empty());
                assert_eq!(
                    app.world().resource::<RenderStatus>().completed_mesh_jobs,
                    completed
                );
                assert_eq!(
                    app.world()
                        .resource::<RenderStatus>()
                        .max_composition_seconds,
                    0.025
                );
                assert_eq!(
                    app.world().resource::<RenderedSections>().sections[&position].entity,
                    root
                );
                assert!(
                    app.world()
                        .get::<crate::transitions::ChunkFade>(root)
                        .is_some()
                );
            }
        }
    }

    #[test]
    fn entering_results_accept_changed_lod_or_micro_detail() {
        for (level, micro_detail) in [
            (super::super::DetailLevel::Middle, true),
            (super::super::DetailLevel::Near, false),
        ] {
            let (mut app, position, old) = entering_upload_fixture();
            app.world_mut()
                .resource_mut::<RenderJobs>()
                .0
                .insert(position, completed_upload_job(level, micro_detail));
            app.update();
            let rendered = &app.world().resource::<RenderedSections>().sections[&position];
            assert_ne!(rendered.entity, old);
            assert!(app.world().get_entity(old).is_err());
            assert_eq!(rendered.detail_level, level);
            assert_eq!(rendered.micro_detail, micro_detail);
            assert_eq!(rendered.meshes.len(), 1);
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
            assert_eq!(
                app.world()
                    .resource::<crate::MeshUploadStatus>()
                    .admitted_vertices,
                3
            );
            let jobs = app.world().resource::<RenderJobs>();
            assert!(jobs.0.is_empty());
            assert!(jobs.1.is_empty());
        }
    }

    #[test]
    fn partially_staged_sections_release_assets_on_edit_unload_and_fixture_replacement() {
        use bevy::ecs::system::RunSystemOnce;

        #[derive(Clone, Copy)]
        enum Abort {
            UploadEdit,
            DispatchEdit,
            Unload,
            Fixture,
        }
        for abort in [
            Abort::UploadEdit,
            Abort::DispatchEdit,
            Abort::Unload,
            Abort::Fixture,
        ] {
            let (mut app, position, old) = entering_upload_fixture();
            let mut job = completed_upload_job(super::super::DetailLevel::Near, false);
            job.ready = Some((
                Ok(worker_batches(
                    (0..2)
                        .map(|_| super::super::builder::MeshBatch {
                            positions: vec![[0.; 3]; crate::upload_budget::TERRAIN_VERTICES],
                            indices: (0..(crate::upload_budget::TERRAIN_VERTICES
                                - crate::upload_budget::TERRAIN_VERTICES % 3)
                                as u32)
                                .collect(),
                            ..default()
                        })
                        .collect(),
                )),
                0.025,
            ));
            app.world_mut()
                .resource_mut::<RenderJobs>()
                .0
                .insert(position, job);
            app.update();
            let staged = app.world().resource::<RenderJobs>().0[&position]
                .ready
                .as_ref()
                .unwrap()
                .0
                .as_ref()
                .unwrap()
                .iter()
                .filter(|batch| batch.staged.is_some())
                .count();
            assert!(staged > 0);
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), staged + 1);
            assert_eq!(app.world().resource::<RenderJobs>().0.len(), 1);
            assert_eq!(
                app.world().resource::<RenderedSections>().sections[&position].entity,
                old
            );
            assert_eq!(
                app.world_mut().query::<&Mesh3d>().iter(app.world()).count(),
                1,
                "private staged batches must not spawn partial scene roots"
            );
            let unloaded = matches!(abort, Abort::Unload);
            match abort {
                Abort::UploadEdit | Abort::DispatchEdit => {
                    app.world_mut()
                        .resource_mut::<EditJournal>()
                        .record([0, 0, 0], 1)
                        .unwrap();
                    if matches!(abort, Abort::DispatchEdit) {
                        let config = terra_core::GameConfig::default();
                        let terrain =
                            terra_world::TerraGenerator::new(config.world.clone()).unwrap();
                        app.world_mut().insert_resource(config);
                        app.world_mut()
                            .insert_resource(terra_world::streaming::WorldTerrain(terrain));
                        app.world_mut()
                            .insert_resource(terra_world::streaming::ChunkFocus([0.; 3]));
                        app.world_mut()
                            .init_resource::<super::super::GeometryCache>();
                        app.world_mut()
                            .run_system_once(super::super::dispatch_meshes)
                            .unwrap();
                    } else {
                        app.update();
                    }
                }
                Abort::Unload => {
                    app.world_mut()
                        .resource_mut::<ChunkStore>()
                        .loaded
                        .remove(&position);
                    app.update();
                }
                Abort::Fixture => {
                    app.world_mut()
                        .init_resource::<super::super::GeometryCache>();
                    super::super::forget_section_snapshots(app.world_mut(), &[position]);
                }
            }
            assert!(app.world().resource::<RenderJobs>().0.is_empty());
            assert_eq!(
                app.world().resource::<Assets<Mesh>>().len(),
                usize::from(!unloaded)
            );
            assert_eq!(app.world().get_entity(old).is_err(), unloaded);
        }
    }

    #[test]
    fn dry_opaque_terrain_uses_shared_depth_correct_material_at_every_lod_without_uv_payload() {
        use super::super::{builder::MeshBatch, jobs::RenderJob};
        use crate::weather::surfaces::{TerrainDepthMaterial, TerrainDepthMaterials};
        for level in [
            super::super::DetailLevel::Near,
            super::super::DetailLevel::Middle,
            super::super::DetailLevel::Far,
        ] {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .init_resource::<Assets<Mesh>>()
                .init_resource::<BlockAssets>()
                .init_resource::<RenderedSections>()
                .init_resource::<RenderStatus>()
                .init_resource::<EditJournal>()
                .init_resource::<ChunkStore>()
                .init_resource::<RenderJobs>()
                .init_resource::<TerrainDepthMaterials>()
                .add_systems(Update, upload_sections);
            app.world_mut()
                .resource_mut::<BlockAssets>()
                .materials
                .push(Handle::default());
            app.world_mut()
                .resource_mut::<TerrainDepthMaterials>()
                .0
                .insert(0, Handle::default());
            let position = terra_voxel::SectionPos::default();
            app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
                position,
                terra_world::streaming::LoadedSection {
                    data: terra_voxel::BlockSection::empty(position).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            app.world_mut().resource_mut::<RenderJobs>().0.insert(
                position,
                RenderJob {
                    ready: Some((
                        Ok(vec![MeshBatch {
                            positions: vec![[0.; 3], [1., 0., 0.], [0., 1., 0.]],
                            indices: vec![0, 1, 2],
                            ..default()
                        }]),
                        0.,
                    )),
                    micro_detail: true,
                    epoch: 0,
                    appearance_epoch: 0,
                    level,
                    task: bevy::tasks::AsyncComputeTaskPool::get().spawn(std::future::pending()),
                },
            );
            app.update();
            let mut query = app
                .world_mut()
                .query_filtered::<&Mesh3d, With<MeshMaterial3d<TerrainDepthMaterial>>>();
            let mesh = query.single(app.world()).unwrap();
            let assets = app.world().resource::<Assets<Mesh>>();
            assert!(
                assets
                    .get(&mesh.0)
                    .unwrap()
                    .attribute(Mesh::ATTRIBUTE_UV_1)
                    .is_none()
            );
            assert_eq!(assets.len(), 1);
            assert_eq!(app.world().resource::<TerrainDepthMaterials>().0.len(), 1);
            assert_eq!(
                app.world().resource::<RenderedSections>().sections[&position].detail_level,
                level
            );
        }
    }

    #[test]
    fn stale_lod_results_do_not_allocate_meshes_or_consume_upload_budget() {
        use super::super::{builder::MeshBatch, jobs::RenderJob};
        let position = terra_voxel::SectionPos::default();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<BlockAssets>()
            .init_resource::<RenderedSections>()
            .init_resource::<RenderStatus>()
            .init_resource::<EditJournal>()
            .init_resource::<ChunkStore>()
            .init_resource::<RenderJobs>()
            .init_resource::<crate::MeshUploadStatus>()
            .insert_resource(terra_world::streaming::ChunkFocus([1000.; 3]))
            .add_systems(Update, upload_sections);
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            position,
            terra_world::streaming::LoadedSection {
                data: terra_voxel::BlockSection::empty(position).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        app.world_mut().resource_mut::<RenderJobs>().0.insert(
            position,
            RenderJob {
                ready: Some((
                    Ok(vec![MeshBatch {
                        positions: vec![[0.; 3]; 3],
                        indices: vec![0, 1, 2],
                        ..default()
                    }]),
                    0.,
                )),
                micro_detail: true,
                epoch: 0,
                appearance_epoch: 0,
                level: super::super::DetailLevel::Near,
                task: bevy::tasks::AsyncComputeTaskPool::get().spawn(std::future::pending()),
            },
        );
        app.update();
        assert!(app.world().resource::<Assets<Mesh>>().is_empty());
        assert!(
            app.world()
                .resource::<RenderedSections>()
                .sections
                .is_empty()
        );
        assert_eq!(
            app.world()
                .resource::<crate::MeshUploadStatus>()
                .admitted_vertices,
            0
        );
        let jobs = app.world().resource::<RenderJobs>();
        assert!(jobs.0.is_empty());
        assert_eq!(jobs.1, [position]);
    }

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
    fn fog_stone_upload_never_casts_shadows_at_any_lod() {
        use super::super::{builder, jobs::RenderJob};
        for fog_id in [399, terra_core::fog_blocks::MATERIAL] {
            let mut templates = crate::blocks::authored_terrain_templates();
            let mut fog = templates.blocks[&1].clone();
            fog.face_materials = [0; 6];
            fog.side_cap = None;
            templates.blocks.insert(fog_id, fog);
            let templates = std::sync::Arc::new(templates);
            let mut padded = [0; terra_voxel::meshing::PADDED_VOLUME];
            padded[1 + 18 * (1 + 18)] = fog_id;
            let geometry = terra_voxel::meshing::mesh_section(&padded).unwrap();
            for level in [
                super::super::DetailLevel::Near,
                super::super::DetailLevel::Middle,
                super::super::DetailLevel::Far,
            ] {
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
                {
                    let mut assets = app.world_mut().resource_mut::<BlockAssets>();
                    assets.templates = Some(templates.clone());
                    assets.materials = vec![Handle::default(); 7];
                }
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
                let mut batches = builder::compose_section_with_options(
                    &geometry.faces,
                    &templates,
                    position,
                    &builder::RenderOptions {
                        water_level: None,
                        detail_level: level,
                        terrain: None,
                        exposure: None,
                        tints: Default::default(),
                        conditions: default(),
                    },
                )
                .unwrap();
                super::super::shadows::append(&mut batches, &geometry.faces, &templates, level)
                    .unwrap();
                assert!(batches.iter().all(|batch| !batch.shadow_only));
                app.world_mut().resource_mut::<RenderJobs>().0.insert(
                    position,
                    RenderJob {
                        ready: Some((Ok(batches), 0.0)),
                        micro_detail: true,
                        epoch: 0,
                        appearance_epoch: 0,
                        level,
                        task: bevy::tasks::AsyncComputeTaskPool::get()
                            .spawn(std::future::pending()),
                    },
                );
                app.update();
                let mut meshes = app
                    .world_mut()
                    .query_filtered::<Option<&bevy::light::NotShadowCaster>, With<Mesh3d>>();
                let markers: Vec<_> = meshes.iter(app.world()).collect();
                assert!(!markers.is_empty());
                assert!(markers.iter().all(|marker| marker.is_some()));
            }
        }
    }

    #[test]
    fn authored_grass_proxy_replaces_dense_caster_and_unloads_owned_meshes() {
        use super::super::{builder, jobs::RenderJob};
        let templates = std::sync::Arc::new(crate::blocks::authored_terrain_templates());
        let mut padded = [0; terra_voxel::meshing::PADDED_VOLUME];
        padded[1 + 18 * (1 + 18)] = 1;
        let geometry = terra_voxel::meshing::mesh_section(&padded).unwrap();
        let position = terra_voxel::SectionPos::default();
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
            .add_systems(Update, upload_sections);
        let materials: Vec<_> = (0..7)
            .map(|_| {
                app.world_mut()
                    .resource_mut::<Assets<StandardMaterial>>()
                    .add(StandardMaterial::default())
            })
            .collect();
        let shadow = materials[0].clone();
        {
            let mut assets = app.world_mut().resource_mut::<BlockAssets>();
            assets.materials = materials;
            assets.templates = Some(templates.clone());
        }
        app.insert_resource(super::super::shadows::OpaqueShadowMaterial(shadow));
        for _ in 0..4 {
            app.insert_resource(crate::MeshUploadStatus::default());
            app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
                position,
                terra_world::streaming::LoadedSection {
                    data: terra_voxel::BlockSection::empty(position).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            app.world_mut()
                .resource_mut::<RenderedSections>()
                .block_revisions
                .insert(position, 1);
            let mut batches = builder::compose_section_with_options(
                &geometry.faces,
                &templates,
                position,
                &builder::RenderOptions {
                    water_level: None,
                    detail_level: super::super::DetailLevel::Near,
                    terrain: None,
                    exposure: None,
                    tints: Default::default(),
                    conditions: default(),
                },
            )
            .unwrap();
            let color_positions = batches[0].positions.clone();
            super::super::shadows::append(
                &mut batches,
                &geometry.faces,
                &templates,
                super::super::DetailLevel::Near,
            )
            .unwrap();
            app.world_mut().resource_mut::<RenderJobs>().0.insert(
                position,
                RenderJob {
                    ready: Some((Ok(batches), 0.0)),
                    micro_detail: true,
                    epoch: 0,
                    appearance_epoch: 0,
                    level: super::super::DetailLevel::Near,
                    task: bevy::tasks::AsyncComputeTaskPool::get().spawn(std::future::pending()),
                },
            );
            app.update();
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
            let color = app
                .world_mut()
                .query_filtered::<Entity, (With<Mesh3d>, Without<super::super::shadows::ShadowProxy>)>()
                .single(app.world())
                .unwrap();
            assert!(
                app.world()
                    .get::<bevy::light::NotShadowCaster>(color)
                    .is_some()
            );
            assert!(
                app.world()
                    .get::<super::super::shadows::ReplacedShadowCaster>(color)
                    .is_some()
            );
            let mesh = app.world().get::<Mesh3d>(color).unwrap();
            assert_eq!(
                app.world()
                    .resource::<Assets<Mesh>>()
                    .get(&mesh.0)
                    .unwrap()
                    .attribute(Mesh::ATTRIBUTE_POSITION),
                Some(&bevy::mesh::VertexAttributeValues::Float32x3(
                    color_positions
                ))
            );
            app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
            app.update();
            assert!(
                app.world()
                    .resource::<RenderedSections>()
                    .sections
                    .is_empty()
            );
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
            assert_eq!(
                app.world_mut().query::<&Mesh3d>().iter(app.world()).count(),
                0
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
                        tints: Default::default(),
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
