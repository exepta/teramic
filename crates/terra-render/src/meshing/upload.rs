use super::{
    RenderStatus, RenderedSection, RenderedSections, TerrainSection,
    jobs::{RenderJobs, poll_job},
};
use crate::blocks::BlockAssets;
use bevy::ecs::system::SystemParam;
use bevy::{camera::primitives::MeshAabb, math::Vec3A, prelude::*};
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
    residents: Query<'w, 's, &'static super::resident::PublishedMeshes>,
    parents: Query<'w, 's, (&'static Visibility, &'static Children)>,
    draw_children: Query<'w, 's, super::published_children::DrawChild>,
}

impl RenderWorld<'_, '_> {
    fn wind_layout(&self, slot: usize, rooted: bool) -> bool {
        self.foliage.as_ref().is_some_and(|assets| {
            assets.materials.contains_key(&slot)
                || assets.flames.contains_key(&slot)
                || assets.reference.contains_key(&slot)
                || rooted && assets.fibres.contains_key(&slot)
        })
    }
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
            .presentations
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
    let mut deferred_new_residents = false;
    let mut positions: Vec<_> = jobs.0.keys().copied().collect();
    positions.sort_by_key(|p| {
        (
            super::jobs::MeshPriority::for_section(*p, &world.journal, &rendered),
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
            let refinement = previous.is_some_and(|old| {
                old.mesh_epoch == job.epoch && old.appearance_epoch == job.appearance_epoch
            });
            let stale_detail = world.focus.as_ref().is_some_and(|focus| {
                super::lod::select_configured_micro_detail(
                    position,
                    focus.0,
                    previous.map(|r| r.micro_detail),
                    client.blocks,
                ) != job.micro_detail
                    || job.folded_leaves.is_some_and(|folded| {
                        folded.at_focus(
                            position,
                            focus.0,
                            previous.and_then(|r| r.folded_leaves),
                            job.level,
                            client.blocks.leaf_geometry_quality,
                        ) != folded
                    })
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
                    && old.folded_leaves == job.folded_leaves
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
                        batch.paired_leaf_shadow = false;
                        batch.profile_leaf_shadow = false;
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
            let mut borrowed_any = false;
            if let (Ok(batches), Ok(origin)) = (&mut result, position.origin()) {
                // The optional far-group experiment needs the original CPU mesh
                // for aggregate construction. Only reuse where that copy is unnecessary.
                let residents = previous
                    .filter(|_| world.far.is_none() || job.level == super::DetailLevel::Near)
                    .and_then(|old| {
                        Some((
                            old.entity,
                            world.residents.get(old.entity).ok()?,
                            old.meshes.as_slice(),
                        ))
                    });
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
                    if let Some(piece) = &mut batch.source {
                        piece.set_wind_layout(
                            world.wind_layout(batch.material, !batch.rooted_wind.is_empty()),
                        );
                    }
                    if let (Some(piece), Some((owner, residents, owned))) =
                        (&batch.source, residents)
                        && let Some(prepared) = residents.find(piece, owned, &meshes)
                    {
                        batch.borrow_resident(owner, prepared);
                        borrowed_any = true;
                        continue;
                    }
                    // No further new uploads in a blocked section, but later
                    // resident matches can still shed their CPU buffers now.
                    if deferred {
                        continue;
                    }
                    if world.budget.as_mut().is_some_and(|budget| {
                        !budget.reserve_terrain_batch(
                            batch.positions.len(),
                            crate::upload_budget::batch_bytes(batch),
                            refinement,
                        )
                    }) {
                        deferred = true;
                        continue;
                    }
                    prepare_batch(
                        batch,
                        &world,
                        &assets,
                        tint.as_ref().map(|t| (t, origin, column.as_deref())),
                        &mut meshes,
                        job.level != super::DetailLevel::Near,
                    );
                }
            }
            if deferred {
                deferred_new_residents |= borrowed_any;
                // Keep prepared assets private until the complete replacement
                // can publish atomically; subsequent frames stage only the rest.
                job.ready = Some((result, seconds));
                if refinement {
                    // New sections can still use the headroom reserved for them.
                    continue;
                }
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
                job.folded_leaves,
                result,
            ));
        }
    }
    for (position, epoch, appearance_epoch, level, micro_detail, folded_leaves, mut result) in
        completed
    {
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
        // Only transfer ownership after the full replacement can commit. Until
        // then stale, cancelled or partial jobs own only their freshly staged assets.
        let previous = rendered.sections.get(&position);
        let lost_resident = batches.iter().any(|batch| {
            let Some(owner) = batch.borrowed_from else {
                return false;
            };
            let staged = batch
                .staged
                .as_ref()
                .expect("resident batch has a staged handle");
            world.far.is_some() && level != super::DetailLevel::Near
                || !world.residents.contains(owner)
                || previous
                    .is_none_or(|old| old.entity != owner || !old.meshes.contains(&staged.handle))
                || !meshes.contains(staged.handle.id())
                || !batch.source.as_ref().is_some_and(|source| {
                    source.supports_wind_layout(world.wind_layout(batch.material, staged.rooted))
                })
        });
        if lost_resident {
            crate::upload_budget::release_staged(&mut batches, &mut meshes);
            jobs.1.push(position);
            continue;
        }
        let edited = super::jobs::interactive_edit(position, &world.journal, &rendered);
        let reused: std::collections::HashSet<_> = batches
            .iter_mut()
            .filter_map(|batch| {
                batch.borrowed_from.take()?;
                let id = batch.staged.as_ref().unwrap().handle.id();
                let source = batch.source.as_ref().unwrap();
                let bytes = source.bytes();
                if source.has_wind_layout() {
                    status.reused_wind_meshes += 1;
                    status.reused_wind_bytes += bytes as u64;
                }
                if batch.shadow_only {
                    status.reused_shadow_meshes += 1;
                    status.reused_shadow_bytes += bytes as u64;
                } else if edited {
                    status.reused_edit_meshes += 1;
                    status.reused_edit_bytes += bytes as u64;
                } else {
                    status.reused_streaming_meshes += 1;
                    status.reused_streaming_bytes += bytes as u64;
                }
                Some(id)
            })
            .collect();
        // Bevy's mesh allocator must not receive an empty vertex/index buffer.
        // Empty section results still retire an obsolete mesh below.
        batches.retain(|batch| batch.staged.is_some());
        // A newly generated neighbor can invalidate this section while its
        // first mesh is still queued/fading. That is still an initial reveal,
        // even with a newer epoch; never expose its replacement fully opaque.
        // Building is an immediate world update, not another generated-chunk
        // entrance. This also covers the first block in a previously empty section.
        let revision = world.journal.revision(position);
        let edited =
            rendered.record_presentation(position, revision, batches.is_empty().then_some(epoch));
        let first_upload = !edited
            && rendered.sections.get(&position).is_none_or(|old| {
                world.queued.contains(old.entity) || world.entering.contains(old.entity)
            });
        if let Some(history) = motion.as_deref_mut() {
            let now = world.time.as_ref().map_or(0.0, |time| time.elapsed_secs());
            for kind in [
                crate::liquid_motion::Kind::Lava,
                crate::liquid_motion::Kind::Galle,
                crate::liquid_motion::Kind::Tar,
            ] {
                let present = batches.iter().any(|batch| {
                    assets.templates.as_ref().and_then(|templates| {
                        crate::liquid_motion::Kind::material(templates, batch.material)
                    }) == Some(kind)
                });
                if present {
                    if kind == crate::liquid_motion::Kind::Lava
                        && !batches.iter().any(|b| b.flowing_lava)
                    {
                        history.remove(crate::liquid_motion::Key(position, kind));
                    }
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
        let mut previous = rendered.sections.remove(&position);
        if let Some(old) = &mut previous {
            if old.mesh_epoch == epoch && old.detail_level != level {
                crate::transitions::record_swap(&mut commands);
            }
            old.meshes.retain(|mesh| !reused.contains(&mesh.id()));
        }
        let origin = position.origin();
        if batches.is_empty() || origin.is_err() {
            if let Some(old) = previous {
                remove_section(old, &mut commands);
            }
            continue;
        }
        let origin = origin.unwrap();
        // Fade/queued materials and optional aggregated far batches have their
        // own ownership. Only transfer live, settled children in the normal path.
        let mut candidates = std::collections::HashMap::<_, Vec<Entity>>::new();
        if !first_upload
            && !reused.is_empty()
            && world.far.is_none()
            && let Some(old) = &previous
            && !world.queued.contains(old.entity)
            && !world.entering.contains(old.entity)
            && let Ok((visibility, children)) = world.parents.get(old.entity)
            && *visibility != Visibility::Hidden
        {
            for child in children.iter() {
                if let Ok(draw) = world.draw_children.get(child)
                    && reused.contains(&draw.mesh.0.id())
                {
                    candidates.entry(draw.mesh.0.id()).or_default().push(child);
                }
            }
        }
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
        let mut residents = super::resident::PublishedMeshes::default();
        for mut batch in batches {
            let shadow_only = batch.shadow_only;
            let mut prepared = batch.staged.take().expect("complete upload is atomic");
            if let Some(piece) = batch.source.take() {
                residents.record(piece, &prepared);
            }
            if let Some(mut mesh) = prepared.liquid.take() {
                if let Some(history) = motion.as_deref_mut()
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
                }
                // This strong, reserved handle has never held an asset. Publish
                // the final geometry once, including when interpolation is absent.
                meshes
                    .insert(prepared.handle.id(), mesh)
                    .expect("staged liquid retains its reserved mesh handle");
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
            let foliage = world.foliage.as_ref().and_then(|a| {
                if shadow_only && batch.profile_leaf_shadow {
                    a.profile_shadows.get(&batch.material)
                } else {
                    a.materials.get(&batch.material)
                }
            });
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
            use super::published_children::{DrawRole, Surface};
            let surface = if prepared.opaque_shadow
                && let Some(material) = world.shadow_material.as_ref()
            {
                Surface::Standard(&material.0)
            } else if let Some(water) = water {
                Surface::Water(&water.0)
            } else if let Some(material) = flame {
                Surface::Flame(material)
            } else if let Some(material) = world
                .foliage
                .as_ref()
                .and_then(|a| a.reference.get(&batch.material))
            {
                Surface::Standard(material)
            } else if let Some(material) = foliage {
                Surface::Leaf(material)
            } else if let Some(material) = fibre {
                Surface::Fibre(material)
            } else if let Some(material) = world
                .wet
                .as_ref()
                .filter(|_| damp)
                .and_then(|wet| wet.0.get(&batch.material))
            {
                Surface::Wet(material)
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
                Surface::Depth(material)
            } else {
                Surface::Standard(&assets.materials[batch.material])
            };
            let permanently_shadowless = assets.templates.as_ref().is_some_and(|t| {
                t.glass_material(batch.material)
                    || t.torch_material(batch.material)
                    || t.fog_material(batch.material)
                    || t.bile_material(batch.material)
            });
            let replaced_caster = !shadow_only
                && ((batch.paired_leaf_shadow && !batch.profile_leaf_shadow)
                    || shadowed_materials.contains(&batch.material));
            let role = DrawRole {
                replaced: replaced_caster && !permanently_shadowless,
                no_shadow: replaced_caster
                    || permanently_shadowless
                    || matches!(surface, Surface::Water(_) | Surface::Flame(_)),
                moving_leaf: matches!(surface, Surface::Leaf(_))
                    && (!batch.paired_leaf_shadow || !shadow_only),
                moving_fibre: matches!(surface, Surface::Fibre(_)),
            };
            if let Some(available) = candidates.get_mut(&handle.id())
                && let Some(old) = &previous
                && let Some(index) = available.iter().position(|child| {
                    world
                        .draw_children
                        .get(*child)
                        .is_ok_and(|draw| role.matches(draw, old.entity, &batch, &surface, bounds))
                })
            {
                // Relationship hooks detach this child from the old root before
                // its retirement traverses and hides the remaining descendants.
                commands
                    .entity(available.swap_remove(index))
                    .insert(ChildOf(root));
                status.retained_draw_entities += 1;
                handles.push(handle);
                continue;
            }
            let mut entity =
                commands.spawn((Mesh3d(handle.clone()), Transform::default(), ChildOf(root)));
            if let Some(coarse) = batch.profile_caster() {
                entity.insert(super::shadows::LeafProfileCaster(coarse));
            }
            if let Some(bounds) = bounds {
                entity.insert((bounds, bevy::camera::visibility::NoAutoAabb));
            }
            if shadow_only {
                entity.insert(super::shadows::ShadowProxy);
                if batch.paired_leaf_shadow {
                    // A camera queues Mesh3d by its visibility class. Lights
                    // query Mesh3d directly, so the compact caster can inherit
                    // *all* original light layers, including miniature worlds.
                    entity.insert((
                        super::shadows::LeafShadowProxy,
                        crate::miniature::layers(),
                        bevy::camera::visibility::VisibilityClass::default(),
                    ));
                } else {
                    entity.insert(bevy::camera::visibility::RenderLayers::layer(
                        super::shadows::LAYER,
                    ));
                }
            } else {
                entity.insert((
                    crate::miniature::layers(),
                    terra_core::miniature::MiniatureMesh::Terrain,
                ));
            }
            surface.insert(&mut entity);
            if matches!(surface, Surface::Water(_)) && coastal_quads > 0 {
                entity.insert(super::CoastalWashQuads(coastal_quads));
            }
            if role.moving_leaf {
                entity.insert(crate::foliage::WindFoliage);
            }
            if role.moving_fibre {
                entity.insert(crate::foliage::WindFibre);
            }
            if role.replaced {
                entity.insert(super::shadows::ReplacedShadowCaster);
            }
            if role.no_shadow {
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
        if let Some(old) = previous {
            remove_section(old, &mut commands);
        }
        residents.insert(&mut commands.entity(root));
        if first_upload {
            crate::transitions::enter(&mut commands, root, animation);
        }
        rendered.sections.insert(
            position,
            RenderedSection {
                folded_leaves,
                micro_detail,
                appearance_epoch,
                entity: root,
                meshes: handles,
                mesh_epoch: epoch,
                detail_level: level,
            },
        );
    }
    if deferred_new_residents {
        let live = jobs
            .0
            .values()
            .filter_map(|job| job.ready.as_ref()?.0.as_ref().ok())
            .flatten()
            .filter(|batch| batch.borrowed_from.is_some())
            .filter_map(|batch| batch.source.as_ref())
            .map(|source| source.bytes())
            .sum();
        status.peak_deferred_resident_bytes = status.peak_deferred_resident_bytes.max(live);
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
    let opaque_shadow = batch.shadow_only
        && !batch.paired_leaf_shadow
        && batch.normals.is_empty()
        && batch.packed_normals.is_empty();
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
    let mut mesh = super::precision::section_mesh(batch)
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
    if !batch.packed_uvs.is_empty() {
        mesh.insert_attribute(
            bevy::mesh::MeshVertexAttribute {
                format: bevy::render::render_resource::VertexFormat::Unorm16x2,
                ..Mesh::ATTRIBUTE_UV_0
            },
            bevy::mesh::VertexAttributeValues::Unorm16x2(std::mem::take(&mut batch.packed_uvs)),
        );
    } else if !batch.uvs.is_empty() {
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
    if let Some((tints, medium_weights)) = tints {
        mesh.insert_attribute(crate::water::biome::ATTRIBUTE_BIOME_TINT, tints);
        mesh.insert_attribute(
            crate::water::biome::ATTRIBUTE_MEDIUM_WEIGHTS,
            medium_weights,
        );
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
    let wind = world.wind_layout(batch.material, rooted);
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
    let liquid_kind = assets
        .templates
        .as_ref()
        .and_then(|templates| crate::liquid_motion::Kind::material(templates, batch.material));
    let far = if world.far.is_some()
        && far
        && !batch.shadow_only
        && !rooted
        && !wind
        && !water
        && assets.templates.is_some()
        && liquid_kind.is_none()
        && world
            .materials
            .as_ref()
            .and_then(|materials| materials.get(&assets.materials[batch.material]))
            .is_some_and(|m| m.alpha_mode == AlphaMode::Opaque)
    {
        world
            .far
            .as_ref()
            .and_then(|cache| cache.capture(&mesh, bounds))
    } else {
        None
    };
    let interpolated = !batch.shadow_only
        && liquid_kind
            .is_some_and(|kind| kind != crate::liquid_motion::Kind::Lava || batch.flowing_lava);
    let (handle, liquid) = if interpolated {
        (meshes.reserve_handle(), Some(mesh))
    } else {
        (meshes.add(mesh), None)
    };
    batch.staged = Some(super::staging::StagedMesh {
        opaque_shadow,
        liquid,
        handle,
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
    use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology};

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
                            Some(&if packed {
                                bevy::mesh::VertexAttributeValues::Snorm8x4(vec![
                                    [0, 0, 127, -127];
                                    3
                                ])
                            } else {
                                bevy::mesh::VertexAttributeValues::Float32x4(tangents)
                            })
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
                        folded_leaves: None,
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
                    folded_leaves: None,
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
                    folded_leaves: None,
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
            folded_leaves: None,
            micro_detail,
            epoch: 0,
            appearance_epoch: 0,
            level,
            task: bevy::tasks::AsyncComputeTaskPool::get().spawn(std::future::pending()),
        }
    }

    #[test]
    fn a_first_presentation_precedes_a_closer_lod_swap_with_one_upload_slot() {
        use super::super::DetailLevel;
        let (mut app, resident, old) = entering_upload_fixture();
        let arrival = resident.offset([4, 0, 0]).unwrap();
        let mut config = terra_core::GameConfig::default();
        config.client.mesh_uploads_per_frame = 1;
        config.client.chunk_animation = terra_core::ChunkAnimation::None;
        app.insert_resource(config);
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            arrival,
            terra_world::streaming::LoadedSection {
                data: terra_voxel::BlockSection::empty(arrival).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let mut jobs = app.world_mut().resource_mut::<RenderJobs>();
        jobs.0
            .insert(resident, completed_upload_job(DetailLevel::Middle, false));
        jobs.0
            .insert(arrival, completed_upload_job(DetailLevel::Near, true));
        app.update();
        let sections = &app.world().resource::<RenderedSections>().sections;
        assert!(sections.contains_key(&arrival));
        assert_eq!(sections[&resident].entity, old);
        assert_eq!(app.world().resource::<RenderJobs>().0.len(), 1);
        app.update();
        let sections = &app.world().resource::<RenderedSections>().sections;
        assert_eq!(sections[&resident].detail_level, DetailLevel::Middle);
        assert!(app.world().resource::<RenderJobs>().0.is_empty());
    }

    #[test]
    fn staged_liquids_publish_once_at_commit_or_release_the_reservation_on_cancel() {
        use super::super::{DetailLevel, builder::MeshBatch};
        use crate::blocks::{BlockTemplate, TemplateSet, definitions::BlockRenderKind};
        use crate::liquid_motion::{FROM, History};
        for (id, flowing, history_at_commit, cancel) in [
            (terra_world::lava::LAVA, true, true, false),
            (terra_core::bile::FLUID, false, false, false),
            (terra_core::roads::WET_TAR, false, true, true),
            (terra_world::lava::LAVA, false, true, false),
        ] {
            let (mut app, position, old) = entering_upload_fixture();
            let mut templates = TemplateSet::default();
            templates.blocks.insert(
                id,
                BlockTemplate {
                    render_kind: BlockRenderKind::Cube,
                    face_materials: [0; 6],
                    faces: Default::default(),
                    side_cap: None,
                    bevel_width: 0.0,
                    surface: None,
                    model: Vec::new(),
                    details: Vec::new(),
                },
            );
            let mut assets = app.world_mut().resource_mut::<BlockAssets>();
            assets.templates = Some(std::sync::Arc::new(templates));
            let other = assets.materials[0].clone();
            assets.materials.push(other);
            let mut job = completed_upload_job(DetailLevel::Near, false);
            let batches = job.ready.as_mut().unwrap().0.as_mut().unwrap();
            batches[0].flowing_lava = flowing;
            let allocation = batches[0].positions.as_ptr() as usize;
            batches.push(MeshBatch {
                material: 1,
                positions: vec![[0.; 3]; 3],
                indices: vec![0, 1, 2],
                ..default()
            });
            app.world_mut()
                .resource_mut::<RenderJobs>()
                .0
                .insert(position, job);
            if !history_at_commit {
                app.world_mut().init_resource::<History>();
            }
            app.world_mut()
                .resource_mut::<crate::MeshUploadStatus>()
                .reserve_batch_bytes(
                    crate::upload_budget::UploadKind::Terrain,
                    crate::upload_budget::TERRAIN_VERTICES - 3,
                    0,
                );
            app.update();
            let job = &app.world().resource::<RenderJobs>().0[&position];
            let staged = job.ready.as_ref().unwrap().0.as_ref().unwrap()[0]
                .staged
                .as_ref()
                .unwrap();
            let deferred = flowing || id != terra_world::lava::LAVA;
            assert_eq!(staged.liquid.is_some(), deferred);
            let handle = staged.handle.clone();
            let meshes = app.world().resource::<Assets<Mesh>>();
            assert_eq!(meshes.contains(handle.id()), !deferred);
            let staged_mesh = staged
                .liquid
                .as_ref()
                .or_else(|| meshes.get(&handle))
                .unwrap();
            assert_eq!(
                staged_mesh
                    .attribute(Mesh::ATTRIBUTE_POSITION)
                    .unwrap()
                    .get_bytes()
                    .as_ptr() as usize,
                allocation
            );
            assert_eq!(
                app.world().resource::<RenderedSections>().sections[&position].entity,
                old
            );
            if history_at_commit {
                app.world_mut().init_resource::<History>();
            } else {
                app.world_mut().remove_resource::<History>();
            }
            if cancel {
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .record([0; 3], 1)
                    .unwrap();
            }
            app.world_mut()
                .insert_resource(crate::MeshUploadStatus::default());
            app.update();
            let meshes = app.world().resource::<Assets<Mesh>>();
            if cancel {
                assert!(!meshes.contains(handle.id()));
                assert_eq!(app.world().resource::<History>().publications, 0);
                assert_eq!(
                    app.world().resource::<RenderedSections>().sections[&position].entity,
                    old
                );
            } else {
                let mesh = meshes.get(&handle).unwrap();
                assert_eq!(
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                        .unwrap()
                        .get_bytes()
                        .as_ptr() as usize,
                    allocation
                );
                assert_eq!(mesh.contains_attribute(FROM), history_at_commit && deferred);
                assert!(
                    app.world().resource::<RenderedSections>().sections[&position]
                        .meshes
                        .contains(&handle)
                );
            }
        }
    }

    #[test]
    fn deferred_resident_drops_streams_and_transfers_only_from_a_live_owner() {
        use super::super::{DetailLevel, builder::MeshBatch, resident::StaticPiece};
        use crate::upload_budget::{MAX_BATCH_BYTES, UploadKind};

        #[derive(Clone, Copy)]
        enum Finish {
            Commit,
            Edit,
            Unload,
            LostHandle,
            LostOwner,
        }
        const BYTES: usize = 384 * 20 + 6;
        for mode in [
            Finish::Commit,
            Finish::Edit,
            Finish::Unload,
            Finish::LostHandle,
            Finish::LostOwner,
        ] {
            let (mut app, position, _) = entering_upload_fixture();
            let templates = std::sync::Arc::new(crate::blocks::TemplateSet::default());
            let queue = |app: &mut App, uvs: &[f32], level| {
                let mut job = completed_upload_job(level, false);
                let batches = job.ready.as_mut().unwrap().0.as_mut().unwrap();
                *batches = uvs
                    .iter()
                    .map(|&u| MeshBatch {
                        positions: vec![[1., 2., 3.]; 384],
                        uvs: vec![[u, 0.5]; 384],
                        compact_indices: vec![0, 1, 2],
                        ..default()
                    })
                    .collect();
                StaticPiece::identify(batches, &templates);
                app.world_mut()
                    .resource_mut::<RenderJobs>()
                    .0
                    .insert(position, job);
                app.insert_resource(crate::MeshUploadStatus::default());
            };
            queue(&mut app, &[0.25], DetailLevel::Middle);
            app.update();
            let old = &app.world().resource::<RenderedSections>().sections[&position];
            let owner = old.entity;
            let resident = old.meshes[0].id();
            queue(&mut app, &[0.5, 0.75, 0.25], DetailLevel::Far);
            assert!(
                app.world_mut()
                    .resource_mut::<crate::MeshUploadStatus>()
                    .reserve_batch_bytes(UploadKind::Terrain, 1, MAX_BATCH_BYTES / 2 - BYTES)
            );
            app.update();
            let batches = app.world().resource::<RenderJobs>().0[&position]
                .ready
                .as_ref()
                .unwrap()
                .0
                .as_ref()
                .unwrap();
            let fresh = batches[0].staged.as_ref().unwrap().handle.id();
            assert!(batches[1].staged.is_none());
            assert_eq!(batches[2].borrowed_from, Some(owner));
            assert_eq!(batches[2].staged.as_ref().unwrap().handle.id(), resident);
            assert_eq!(crate::upload_budget::batch_bytes(&batches[2]), 0);
            assert_eq!(batches[2].positions.capacity(), 0);
            assert_eq!(
                app.world()
                    .resource::<RenderStatus>()
                    .peak_deferred_resident_bytes,
                BYTES
            );
            // A second blocked frame neither stages nor transfers the handle.
            app.update();
            assert_eq!(
                app.world().resource::<RenderedSections>().sections[&position].meshes[0].id(),
                resident
            );
            app.insert_resource(crate::MeshUploadStatus::default());
            match mode {
                Finish::Commit => {}
                Finish::Edit => {
                    app.world_mut()
                        .resource_mut::<EditJournal>()
                        .record([0, 0, 0], 1)
                        .unwrap();
                }
                Finish::Unload => {
                    app.world_mut()
                        .resource_mut::<ChunkStore>()
                        .loaded
                        .remove(&position);
                }
                Finish::LostHandle => {
                    app.world_mut()
                        .resource_mut::<Assets<Mesh>>()
                        .remove(resident);
                }
                Finish::LostOwner => {
                    let replacement = app
                        .world_mut()
                        .spawn(super::super::resident::PublishedMeshes::default())
                        .id();
                    app.world_mut()
                        .resource_mut::<RenderedSections>()
                        .sections
                        .get_mut(&position)
                        .unwrap()
                        .entity = replacement;
                }
            }
            app.update();
            let jobs = app.world().resource::<RenderJobs>();
            assert!(jobs.0.is_empty());
            let assets = app.world().resource::<Assets<Mesh>>();
            assert_eq!(assets.contains(fresh), matches!(mode, Finish::Commit));
            assert_eq!(
                assets.contains(resident),
                !matches!(mode, Finish::Unload | Finish::LostHandle)
            );
            if matches!(mode, Finish::Commit) {
                let new = &app.world().resource::<RenderedSections>().sections[&position];
                assert_ne!(new.entity, owner);
                assert_eq!(
                    new.meshes
                        .iter()
                        .filter(|mesh| mesh.id() == resident)
                        .count(),
                    1
                );
                assert_eq!(assets.len(), 3);
                assert_eq!(
                    app.world()
                        .resource::<RenderStatus>()
                        .reused_streaming_meshes,
                    1
                );
            } else if matches!(mode, Finish::LostHandle | Finish::LostOwner | Finish::Edit) {
                assert!(jobs.1.contains(&position));
                assert_eq!(
                    app.world()
                        .resource::<RenderStatus>()
                        .reused_streaming_meshes,
                    0
                );
            }
        }
    }

    #[test]
    fn streamed_section_reuses_the_unchanged_gpu_mesh_only_after_atomic_publication() {
        use super::super::{DetailLevel, builder::MeshBatch, resident::StaticPiece};
        let (mut app, position, _) = entering_upload_fixture();
        let templates = std::sync::Arc::new(crate::blocks::TemplateSet::default());
        let queue = |app: &mut App, uv: f32, level| {
            let mut job = completed_upload_job(level, false);
            let batches = job.ready.as_mut().unwrap().0.as_mut().unwrap();
            *batches = [0.25, uv]
                .into_iter()
                .map(|u| MeshBatch {
                    positions: vec![[1., 2., 3.]; 384],
                    uvs: vec![[u, 0.5]; 384],
                    compact_indices: vec![0, 1, 2],
                    ..default()
                })
                .collect();
            StaticPiece::identify(batches, &templates);
            app.world_mut()
                .resource_mut::<RenderJobs>()
                .0
                .insert(position, job);
            app.world_mut()
                .insert_resource(crate::MeshUploadStatus::default());
        };
        let handles = |app: &App| {
            app.world().resource::<RenderedSections>().sections[&position]
                .meshes
                .iter()
                .map(Handle::id)
                .collect::<Vec<_>>()
        };
        queue(&mut app, 0.5, DetailLevel::Middle);
        app.update();
        let original = handles(&app);
        let first_bounds = |app: &App| {
            let root = app.world().resource::<RenderedSections>().sections[&position].entity;
            let child = app.world().get::<Children>(root).unwrap()[0];
            *app.world()
                .get::<bevy::camera::primitives::Aabb>(child)
                .unwrap()
        };
        let original_bounds = first_bounds(&app);
        queue(&mut app, 0.75, DetailLevel::Far);
        assert!(
            app.world_mut()
                .resource_mut::<crate::MeshUploadStatus>()
                .reserve_batch_bytes(
                    crate::upload_budget::UploadKind::Terrain,
                    1,
                    crate::upload_budget::TERRAIN_BYTES
                )
        );
        app.update();
        assert_eq!(handles(&app), original);
        app.world_mut()
            .init_resource::<super::super::GeometryCache>();
        super::super::forget_section_snapshots(app.world_mut(), &[position]);
        assert!(
            original
                .iter()
                .all(|id| app.world().resource::<Assets<Mesh>>().contains(*id))
        );
        queue(&mut app, 0.75, DetailLevel::Far);
        app.update();
        let replacement = handles(&app);
        assert_eq!(replacement[0], original[0]);
        assert_eq!(first_bounds(&app), original_bounds);
        assert_eq!(
            app.world().resource::<RenderedSections>().sections[&position].detail_level,
            DetailLevel::Far,
        );
        assert_ne!(replacement[1], original[1]);
        assert!(!app.world().resource::<Assets<Mesh>>().contains(original[1]));
        let status = app.world().resource::<RenderStatus>();
        assert_eq!(
            (
                status.reused_streaming_meshes,
                status.reused_streaming_bytes
            ),
            (1, 384 * 20 + 6)
        );
        assert_eq!(
            app.world()
                .resource::<crate::MeshUploadStatus>()
                .admitted_vertices,
            384
        );
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .remove(&position);
        app.update();
        assert!(app.world().resource::<Assets<Mesh>>().is_empty());
    }

    #[test]
    fn leaf_shadow_upload_pairs_only_its_color_piece_and_keeps_all_world_light_layers() {
        use super::super::{
            DetailLevel,
            builder::MeshBatch,
            shadows::{LeafShadowProxy, ReplacedShadowCaster, append_leaf_shadows},
        };
        use crate::foliage::{FoliageAssets, LeafMaterial};
        use bevy::camera::visibility::{RenderLayers, VisibilityClass};
        use bevy::light::NotShadowCaster;
        let (mut app, position, _) = entering_upload_fixture();
        let mut leaves = FoliageAssets::default();
        leaves.materials.insert(0, Handle::default());
        app.insert_resource(leaves);
        let mut job = completed_upload_job(DetailLevel::Middle, false);
        let batches = job.ready.as_mut().unwrap().0.as_mut().unwrap();
        *batches = [384, 3]
            .map(|count| MeshBatch {
                positions: (0..count).map(|i| [f32::from(i % 3), 0., 1.]).collect(),
                packed_colors: vec![[255, 10, 20, 105]; usize::from(count)],
                compact_indices: (0..count).collect(),
                ..default()
            })
            .into();
        append_leaf_shadows(batches, &std::collections::HashSet::from([0]), false).unwrap();
        app.world_mut()
            .resource_mut::<RenderJobs>()
            .0
            .insert(position, job);
        app.update();
        let root = app.world().resource::<RenderedSections>().sections[&position].entity;
        let children = app.world().get::<Children>(root).unwrap();
        assert_eq!(children.len(), 3);
        for &entity in &children[..] {
            assert!(
                app.world()
                    .get::<MeshMaterial3d<LeafMaterial>>(entity)
                    .is_some()
            );
        }
        let (replaced, other, proxy) = (children[0], children[1], children[2]);
        assert!(app.world().get::<ReplacedShadowCaster>(replaced).is_some());
        assert!(app.world().get::<NotShadowCaster>(replaced).is_some());
        assert!(app.world().get::<NotShadowCaster>(other).is_none());
        assert!(app.world().get::<LeafShadowProxy>(proxy).is_some());
        assert!(app.world().get::<NotShadowCaster>(proxy).is_none());
        assert!(
            app.world()
                .get::<VisibilityClass>(proxy)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            app.world().get::<RenderLayers>(proxy),
            app.world().get::<RenderLayers>(replaced)
        );
        assert_eq!(
            app.world().get::<bevy::camera::primitives::Aabb>(proxy),
            app.world().get::<bevy::camera::primitives::Aabb>(replaced)
        );
    }

    #[test]
    fn crowns_and_rooted_fibres_reuse_only_the_same_wind_geometry_and_padded_layout() {
        use super::super::{DetailLevel, builder::MeshBatch, resident::StaticPiece};
        use crate::foliage::{FoliageAssets, WIND_BOUNDS, WindFibre, WindFoliage};
        let (mut app, position, _) = entering_upload_fixture();
        let mut templates = crate::blocks::TemplateSet::default();
        templates.foliage_clusters.insert(
            7,
            crate::blocks::FoliageClusterMaterials {
                leaves: 0,
                needles: None,
            },
        );
        let templates = std::sync::Arc::new(templates);
        let mut assets = app.world_mut().resource_mut::<BlockAssets>();
        assets.templates = Some(templates.clone());
        let base = assets.materials[0].clone();
        assets.materials.push(base);
        let publish = |app: &mut App, level, seasonal_alpha: u8, weight: f32| {
            let mut job = completed_upload_job(level, false);
            let batches = job.ready.as_mut().unwrap().0.as_mut().unwrap();
            *batches = (0..2)
                .map(|material| MeshBatch {
                    material,
                    positions: vec![[1., 2., 3.]; 384],
                    packed_colors: vec![[120, 159, 132, seasonal_alpha]; 384],
                    rooted_wind: if material == 1 {
                        vec![[0., weight]; 384]
                    } else {
                        Vec::new()
                    },
                    compact_indices: vec![0, 1, 2],
                    ..default()
                })
                .collect();
            StaticPiece::identify(batches, &templates);
            app.world_mut()
                .resource_mut::<RenderJobs>()
                .0
                .insert(position, job);
            app.world_mut()
                .insert_resource(crate::MeshUploadStatus::default());
            app.update();
            app.world().resource::<RenderedSections>().sections[&position]
                .meshes
                .iter()
                .map(Handle::id)
                .collect::<Vec<_>>()
        };

        let draw_ids = |app: &App| {
            let root = app.world().resource::<RenderedSections>().sections[&position].entity;
            app.world()
                .get::<Children>(root)
                .unwrap()
                .iter()
                .collect::<Vec<_>>()
        };
        let unpadded = publish(&mut app, DetailLevel::Middle, 62, 0.5);
        app.init_resource::<FoliageAssets>();
        let mut leaves = app.world_mut().resource_mut::<FoliageAssets>();
        leaves.materials.insert(0, Handle::default());
        leaves.fibres.insert(1, Handle::default());
        let padded = publish(&mut app, DetailLevel::Middle, 62, 0.5);
        assert!(unpadded.iter().zip(&padded).all(|(a, b)| a != b));
        let original = draw_ids(&app);
        assert_eq!(publish(&mut app, DetailLevel::Near, 62, 0.5), padded);
        assert_eq!(draw_ids(&app), original);
        assert_eq!(
            app.world()
                .resource::<RenderStatus>()
                .retained_draw_entities,
            2
        );
        let root = app.world().resource::<RenderedSections>().sections[&position].entity;
        let children = app.world().get::<Children>(root).unwrap();
        assert!(app.world().get::<WindFoliage>(children[0]).is_some());
        assert!(app.world().get::<WindFibre>(children[1]).is_some());
        for child in children.iter() {
            assert_eq!(
                app.world()
                    .get::<bevy::camera::primitives::Aabb>(child)
                    .unwrap()
                    .half_extents,
                Vec3A::splat(WIND_BOUNDS)
            );
        }
        assert_eq!(app.world().resource::<RenderStatus>().reused_wind_meshes, 2);
        assert_eq!(
            app.world()
                .resource::<crate::MeshUploadStatus>()
                .admitted_vertices,
            0
        );
        // A changed binding or runtime layer cannot inherit the old draw
        // entity, even when the GPU geometry remains identical.
        let other = Handle::<crate::foliage::LeafMaterial>::Uuid(
            bevy::asset::uuid::Uuid::from_u128(731640),
            std::marker::PhantomData,
        );
        app.world_mut()
            .resource_mut::<FoliageAssets>()
            .materials
            .insert(0, other);
        app.world_mut()
            .entity_mut(original[1])
            .insert(bevy::camera::visibility::RenderLayers::layer(31));
        assert_eq!(publish(&mut app, DetailLevel::Middle, 62, 0.5), padded);
        let rebound = draw_ids(&app);
        assert_ne!(rebound[0], original[0]);
        assert_ne!(rebound[1], original[1]);
        let changed_weight = publish(&mut app, DetailLevel::Near, 62, 0.2);
        assert_eq!(changed_weight[0], padded[0]);
        assert_eq!(draw_ids(&app)[0], rebound[0]);
        assert_ne!(draw_ids(&app)[1], rebound[1]);
        assert_ne!(changed_weight[1], padded[1]);
        let changed_season = publish(&mut app, DetailLevel::Near, 95, 0.2);
        assert!(
            changed_weight
                .iter()
                .zip(&changed_season)
                .all(|(a, b)| a != b)
        );
    }

    #[test]
    fn near_shadow_reuse_survives_partial_cancellation_and_retires_on_new_geometry_or_lod() {
        use super::super::{DetailLevel, builder::MeshBatch, resident::ShadowPiece};
        use std::sync::Arc;
        let (mut app, position, _) = entering_upload_fixture();
        let mut geometry = Arc::new(terra_voxel::meshing::BlockMesh::default());
        let mut templates = Arc::new(crate::blocks::TemplateSet::default());
        let queue = |app: &mut App, geometry: &Arc<_>, templates: &Arc<_>, level| {
            let mut job = completed_upload_job(level, false);
            let batches = job.ready.as_mut().unwrap().0.as_mut().unwrap();
            batches.insert(
                0,
                MeshBatch {
                    shadow_only: true,
                    shadow_sources: vec![0],
                    positions: vec![[0.; 3], [1., 0., 0.], [0., 1., 0.]],
                    compact_indices: vec![0, 1, 2],
                    ..default()
                },
            );
            if level == DetailLevel::Near {
                ShadowPiece::identify(batches, geometry, templates);
            }
            app.world_mut()
                .resource_mut::<RenderJobs>()
                .0
                .insert(position, job);
            app.world_mut()
                .insert_resource(crate::MeshUploadStatus::default());
        };
        let mesh = |app: &App| {
            app.world().resource::<RenderedSections>().sections[&position].meshes[0].id()
        };
        queue(&mut app, &geometry, &templates, DetailLevel::Near);
        app.update();
        let original = mesh(&app);
        queue(&mut app, &geometry, &templates, DetailLevel::Near);
        assert!(
            app.world_mut()
                .resource_mut::<crate::MeshUploadStatus>()
                .reserve_batch_bytes(
                    crate::upload_budget::UploadKind::Terrain,
                    1,
                    crate::upload_budget::TERRAIN_BYTES
                )
        );
        app.update();
        assert!(
            app.world()
                .resource::<RenderJobs>()
                .0
                .contains_key(&position)
        );
        app.world_mut()
            .init_resource::<super::super::GeometryCache>();
        super::super::forget_section_snapshots(app.world_mut(), &[position]);
        assert!(app.world().resource::<Assets<Mesh>>().contains(original));
        assert_eq!(
            app.world().resource::<RenderStatus>().reused_shadow_meshes,
            0
        );
        queue(&mut app, &geometry, &templates, DetailLevel::Near);
        app.update();
        assert_eq!(mesh(&app), original);
        assert_eq!(
            app.world()
                .resource::<crate::MeshUploadStatus>()
                .admitted_vertices,
            3
        );
        let status = app.world().resource::<RenderStatus>();
        assert_eq!(
            (status.reused_shadow_meshes, status.reused_shadow_bytes),
            (1, 42)
        );
        for change in 0..3 {
            let previous = mesh(&app);
            let level = match change {
                0 => {
                    geometry = Arc::new(Default::default());
                    DetailLevel::Near
                }
                1 => {
                    templates = Arc::new(Default::default());
                    DetailLevel::Near
                }
                _ => DetailLevel::Middle,
            };
            queue(&mut app, &geometry, &templates, level);
            app.update();
            assert_ne!(mesh(&app), previous);
            assert!(!app.world().resource::<Assets<Mesh>>().contains(previous));
            assert_eq!(
                app.world()
                    .resource::<crate::MeshUploadStatus>()
                    .admitted_vertices,
                6
            );
        }
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .remove(&position);
        app.update();
        assert!(app.world().resource::<Assets<Mesh>>().is_empty());
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
    fn an_octant_change_rejects_a_completed_leaf_job_while_the_section_stays_near() {
        use super::super::lod::{DetailLevel::Near, select_folded_leaves};
        let (mut app, position, old) = entering_upload_fixture();
        let quality = terra_core::BlockDetailQuality::High;
        let before = select_folded_leaves(position, [8.; 3], None, Near, quality, u8::MAX);
        // The west half is 34 m from the old focus band's east edge, but the
        // east half still uses full folds. The whole section stays Near.
        let focus = [43., 8., 8.];
        let after = before.at_focus(position, focus, None, Near, quality);
        assert!(before.nearby && after.nearby && before != after);
        let mut job = completed_upload_job(Near, true);
        job.folded_leaves = Some(before);
        app.insert_resource(terra_world::streaming::ChunkFocus(focus));
        app.world_mut()
            .resource_mut::<RenderJobs>()
            .0
            .insert(position, job);
        app.update();
        assert_eq!(
            app.world().resource::<RenderedSections>().sections[&position].entity,
            old
        );
        assert!(app.world().resource::<RenderJobs>().0.is_empty());
        assert_eq!(
            app.world()
                .resource::<crate::MeshUploadStatus>()
                .admitted_vertices,
            0
        );
    }

    #[test]
    fn entering_results_accept_changed_lod_micro_detail_or_crown_folds() {
        for (level, micro_detail, folded_leaves) in [
            (super::super::DetailLevel::Middle, true, None),
            (super::super::DetailLevel::Near, false, None),
            (
                super::super::DetailLevel::Near,
                true,
                Some(super::super::lod::FoldedLeaves::FULL),
            ),
        ] {
            let (mut app, position, old) = entering_upload_fixture();
            let mut job = completed_upload_job(level, micro_detail);
            job.folded_leaves = folded_leaves;
            app.world_mut()
                .resource_mut::<RenderJobs>()
                .0
                .insert(position, job);
            app.update();
            let rendered = &app.world().resource::<RenderedSections>().sections[&position];
            assert_ne!(rendered.entity, old);
            assert!(app.world().get_entity(old).is_err());
            assert_eq!(rendered.detail_level, level);
            assert_eq!(rendered.micro_detail, micro_detail);
            assert_eq!(rendered.folded_leaves, folded_leaves);
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
                    folded_leaves: None,
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
                folded_leaves: None,
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
                    folded_leaves: None,
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
                        folded_leaves: None,
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
    fn legacy_authored_grass_proxy_replaces_dense_caster_and_unloads_owned_meshes() {
        use super::super::{builder, jobs::RenderJob};
        let mut legacy = crate::blocks::authored_terrain_templates();
        legacy.sculpted_blocks.clear();
        let templates = std::sync::Arc::new(legacy);
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
                .record_presentation(position, 1, None);
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
                    folded_leaves: None,
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
                    folded_leaves: None,
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
