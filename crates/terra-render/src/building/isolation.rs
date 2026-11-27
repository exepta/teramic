//! One temporary, asynchronous render-only section replacement during mining.
use super::{BuildingStatus, BuildingWorld, mining_geometry};
use crate::meshing::{
    DetailLevel, GeometryCache, RenderedSections,
    builder::{MeshBatch, RenderOptions, compose_section_with_leaf_detail},
};
use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use terra_network::protocol::session::MiningTarget;
use terra_voxel::SectionPos;
use terra_world::{edits::EditJournal, streaming::WorldTerrain};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Key {
    position: [i64; 3],
    owner: [i64; 3],
    material: u16,
    model: bool,
    object: terra_world::edits::BlockObjectState,
    half: Option<terra_world::slabs::HalfRemoval>,
    section: SectionPos,
    original: Entity,
    epoch: u64,
    appearance: u64,
    level: DetailLevel,
    micro: bool,
    folded_leaves: Option<crate::meshing::lod::FoldedLeaves>,
    neighbors: [u16; 6],
}
struct Geometry {
    section: Vec<MeshBatch>,
    block: Vec<MeshBatch>,
    rim: Vec<MeshBatch>,
    model_pieces: Vec<(Vec3, Vec<MeshBatch>)>,
}
struct Active {
    key: Key,
    root: Entity,
    target_root: Option<Entity>,
    meshes: Vec<Handle<Mesh>>,
}
#[derive(Resource, Default)]
pub(super) struct Isolation {
    pending: Option<(Key, Task<Result<Geometry, String>>)>,
    active: Option<Active>,
    failed: Option<Key>,
}

#[derive(Component)]
pub(super) struct MiningPiece {
    base: Vec3,
    offset: Vec3,
    scale: f32,
    order: usize,
    model: bool,
}

#[derive(SystemParam)]
pub(super) struct Scene<'w> {
    rendered: Option<Res<'w, RenderedSections>>,
    cache: Option<Res<'w, GeometryCache>>,
    journal: Option<Res<'w, EditJournal>>,
    terrain: Option<Res<'w, WorldTerrain>>,
    config: Option<Res<'w, terra_core::GameConfig>>,
    foliage: Option<Res<'w, crate::foliage::FoliageAssets>>,
    water: Option<Res<'w, crate::particle_fluids::HydraulicMaterials>>,
}

fn requested(world: &BuildingWorld, scene: &Scene) -> Option<Key> {
    if !world.active() || !world.input.captured || world.blocked.0 || world.mining.progress <= 0.0 {
        return None;
    }
    let MiningTarget::Block { position, expected } = world.mining.target? else {
        return None;
    };
    // Hinged furniture is rendered outside terrain batches. Isolating its
    // section would hide unrelated terrain without removing the animated leaf.
    if terra_world::objects::is_opening(expected) || terra_core::rails::is_rail(expected) {
        // Rail surfaces belong to several section slices. A one-cell isolation
        // would leave those slices behind and duplicate the complete module.
        return None;
    }
    let journal = scene.journal.as_ref()?;
    let template = world.blocks.templates.as_ref()?.blocks.get(&expected)?;
    let model = !template.model.is_empty();
    let selected_state = journal.object_state(position).unwrap_or_default();
    let owner = if model && selected_state.secondary {
        selected_state.peer?
    } else {
        position
    };
    let object = journal.object_state(owner).unwrap_or_default();
    let (section, _) = terra_voxel::blocks::global_to_section(owner);
    let rendered = scene.rendered.as_ref()?.sections.get(&section)?;
    if world.chunks.block_at(position) != Some(expected)
        || rendered.detail_level != DetailLevel::Near
        || rendered.mesh_epoch != journal.mesh_epoch(section)
        || rendered.appearance_epoch != scene.cache.as_ref()?.appearance_epoch(section, journal)
    {
        return None;
    }
    let mut neighbors = [0; 6];
    for (side, material) in neighbors.iter_mut().enumerate().filter(|_| !model) {
        let mut neighbor = position;
        neighbor[side / 2] += if side % 2 == 0 { -1 } else { 1 };
        *material = world.chunks.block_at(neighbor)?;
    }
    Some(Key {
        position,
        owner,
        material: expected,
        model,
        object,
        half: world.mining.slab_half(),
        section,
        original: rendered.entity,
        epoch: rendered.mesh_epoch,
        appearance: rendered.appearance_epoch,
        level: rendered.detail_level,
        micro: rendered.micro_detail,
        folded_leaves: rendered.folded_leaves,
        neighbors,
    })
}

pub(super) fn update(
    mut commands: Commands,
    world: BuildingWorld,
    scene: Scene,
    mut state: ResMut<Isolation>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut visible: Query<&mut Visibility, Without<MiningPiece>>,
    mut status: ResMut<BuildingStatus>,
) {
    let key = requested(&world, &scene);
    if let Some(active) = &mut state.active {
        let original_is_current = scene
            .rendered
            .as_ref()
            .and_then(|rendered| rendered.sections.get(&active.key.section))
            .is_some_and(|rendered| {
                rendered.entity == active.key.original && rendered.mesh_epoch == active.key.epoch
            });
        if original_is_current
            && world.active()
            && world.chunks.loaded.contains_key(&active.key.section)
            && (active.target_root.is_none()
                || world.chunks.block_at(active.key.position) == Some(0)
                || active.key.half.is_some_and(|half| {
                    world.chunks.block_at(active.key.position) == Some(half.remaining)
                        && scene.journal.as_ref().is_some_and(|journal| {
                            journal.object_state(active.key.position)
                                == Some(half.remaining_state(active.key.object))
                        })
                }))
        {
            // Authority committed before the next ordinary mesh upload. Keep the
            // replacement terrain, but remove every cosmetic part of the target.
            if let Some(target) = active.target_root.take() {
                commands.entity(target).despawn();
            }
            state.pending = None;
            status.isolated_block = false;
            return;
        }
    }
    if state
        .active
        .as_ref()
        .is_some_and(|active| Some(active.key) != key)
        && let Some(active) = state.active.take()
    {
        if let Ok(mut visibility) = visible.get_mut(active.key.original) {
            *visibility = Visibility::Inherited;
        }
        commands.entity(active.root).despawn();
        for handle in active.meshes {
            meshes.remove(handle.id());
        }
    }
    if state
        .pending
        .as_ref()
        .is_some_and(|(pending, _)| Some(*pending) != key)
    {
        state.pending = None;
    }
    if state.failed != key {
        state.failed = None;
    }
    status.isolated_block = state
        .active
        .as_ref()
        .is_some_and(|active| active.target_root.is_some());
    let Some(key) = key else {
        return;
    };
    if state.active.is_some() || state.failed == Some(key) {
        return;
    }
    if let Some((_, task)) = &mut state.pending {
        let Some(result) = future::block_on(future::poll_once(task)) else {
            return;
        };
        state.pending = None;
        let geometry = match result {
            Ok(geometry) => geometry,
            Err(error) => {
                tracing::warn!(%error, "Mining section preparation failed");
                state.failed = Some(key);
                return;
            }
        };
        let Ok(mut original) = visible.get_mut(key.original) else {
            return;
        };
        let center = Vec3::from_array(key.position.map(|v| v as f32 + 0.5));
        let Ok(origin) = key.section.origin() else {
            return;
        };
        let root = commands
            .spawn((
                Transform::default(),
                Visibility::default(),
                Name::new("Mining render replacement"),
            ))
            .id();
        let mut handles = Vec::new();
        for batch in geometry.section {
            let handle = meshes.add(crate::previews::batch_mesh(&batch));
            spawn_batch(
                &mut commands,
                &world,
                &scene,
                root,
                batch.material,
                handle.clone(),
                Transform::from_translation(Vec3::from_array(origin.map(|v| v as f32))),
            );
            handles.push(handle);
        }
        let target_root = commands
            .spawn((Transform::default(), Visibility::default(), ChildOf(root)))
            .id();
        for batch in geometry.rim {
            let handle = meshes.add(crate::previews::batch_mesh(&batch));
            spawn_batch(
                &mut commands,
                &world,
                &scene,
                target_root,
                batch.material,
                handle.clone(),
                Transform::from_translation(center),
            );
            handles.push(handle);
        }
        for batch in geometry.block {
            let handle = meshes.add(crate::previews::batch_mesh(&batch));
            for index in 0..9 {
                let (offset, scale, order) = piece_layout(index);
                let entity = spawn_batch(
                    &mut commands,
                    &world,
                    &scene,
                    target_root,
                    batch.material,
                    handle.clone(),
                    Transform::from_translation(center + offset).with_scale(Vec3::splat(scale)),
                );
                commands.entity(entity).insert(MiningPiece {
                    base: center,
                    offset,
                    scale,
                    order,
                    model: false,
                });
            }
            handles.push(handle);
        }
        for (index, (pivot, batches)) in geometry.model_pieces.into_iter().enumerate() {
            for batch in batches {
                let handle = meshes.add(crate::previews::batch_mesh(&batch));
                let base = Vec3::from_array(origin.map(|v| v as f32)) + pivot;
                let entity = spawn_batch(
                    &mut commands,
                    &world,
                    &scene,
                    target_root,
                    batch.material,
                    handle.clone(),
                    Transform::from_translation(base),
                );
                commands.entity(entity).insert(MiningPiece {
                    base,
                    offset: pivot
                        - Vec3::from_array(key.owner.map(|v| v.rem_euclid(16) as f32 + 0.5)),
                    scale: 1.0,
                    order: (index * 5 + 3) % 8,
                    model: true,
                });
                handles.push(handle);
            }
        }
        // Hide only after all replacement meshes are ready, in the same deferred commit.
        *original = Visibility::Hidden;
        state.active = Some(Active {
            key,
            root,
            target_root: Some(target_root),
            meshes: handles,
        });
        status.isolated_block = true;
        return;
    }
    let Some(cache) = &scene.cache else {
        return;
    };
    let Some((mut geometry, models)) = cache.snapshot(key.section, key.epoch) else {
        return;
    };
    let Some(source) = &world.blocks.templates else {
        return;
    };
    let mut templates = source.as_ref().clone();
    let folded = key
        .folded_leaves
        .unwrap_or(crate::meshing::lod::FoldedLeaves::FULL);
    if !key.micro {
        templates.block_details.micro_geometry = false;
        for block in templates.blocks.values_mut() {
            block.surface = None;
        }
    }
    let options = RenderOptions {
        detail_level: key.level,
        water_level: scene
            .config
            .as_ref()
            .and_then(|config| config.world.water.then_some(config.world.sea_level)),
        terrain: scene.terrain.as_ref().map(|terrain| terrain.0.clone()),
        exposure: None,
        tints: std::sync::Arc::new(
            scene
                .journal
                .as_ref()
                .map(|journal| journal.tint_snapshot(key.section))
                .unwrap_or_default(),
        ),
        conditions: scene
            .journal
            .as_ref()
            .map(|journal| journal.surface_snapshot(key.section))
            .unwrap_or_default(),
    };
    let cell = key.owner.map(|v| v.rem_euclid(16) as u32);
    let edits = scene
        .journal
        .as_ref()
        .map(|journal| journal.snapshot(key.section))
        .transpose();
    let neighbor_tints = std::array::from_fn(|side| {
        let mut neighbor = key.position;
        neighbor[side / 2] += if side % 2 == 0 { -1 } else { 1 };
        scene
            .journal
            .as_ref()
            .and_then(|journal| journal.tint_at(neighbor))
    });
    state.pending = Some((
        key,
        AsyncComputeTaskPool::get().spawn(async move {
            if let Some(edits) = edits.map_err(|error| error.to_string())?
                && let Some(terrain) = &options.terrain
            {
                crate::meshing::refresh_growth(&mut geometry, key.section, terrain, &edits);
            }
            if key.model {
                let snapshot = models.as_ref().ok_or("Missing model mining snapshot")?;
                let (mut section, mut target) = crate::meshing::custom::isolate_model(
                    &geometry,
                    snapshot,
                    &templates,
                    key.section,
                    &options,
                    folded,
                    cell,
                    key.half,
                )?;
                // A double chest shares its rendered shell, not its logical
                // selection. Keep the unmined half in the replacement terrain.
                if key.material == terra_voxel::MaterialId::Chest as u16
                    && let Some(peer) = key.object.peer
                {
                    let axis = (0..3)
                        .find(|&axis| peer[axis] != key.owner[axis])
                        .ok_or("Invalid chest footprint")?;
                    let plane = cell[axis] as f32
                        + if peer[axis] > key.owner[axis] {
                            1.0
                        } else {
                            0.0
                        };
                    let selected_less = if key.position == key.owner {
                        peer[axis] > key.owner[axis]
                    } else {
                        peer[axis] < key.owner[axis]
                    };
                    section.extend(super::model_pieces::clip(
                        &target,
                        axis,
                        plane,
                        !selected_less,
                    )?);
                    target = super::model_pieces::clip(&target, axis, plane, selected_less)?;
                }
                return Ok(Geometry {
                    section,
                    block: Vec::new(),
                    rim: Vec::new(),
                    model_pieces: super::model_pieces::partition(target)?,
                });
            }
            let faces = mining_geometry::without_cell(&geometry.faces, cell);
            let remaining_segments: Vec<_> = geometry
                .segments
                .iter()
                .filter(|segment| segment.cell.map(u32::from) != cell)
                .copied()
                .collect();
            let selected_segments: Vec<_> = geometry
                .segments
                .iter()
                .filter(|segment| segment.cell.map(u32::from) == cell)
                .copied()
                .collect();
            let mut section = if let Some(models) = &models {
                crate::meshing::custom::compose_without_cell(
                    &terra_voxel::meshing::BlockMesh {
                        faces,
                        mineral_growth: geometry.mineral_growth.clone(),
                        segments: remaining_segments,
                        ..Default::default()
                    },
                    models,
                    &templates,
                    key.section,
                    &options,
                    folded,
                    cell,
                )?
            } else {
                compose_section_with_leaf_detail(
                    &faces,
                    &templates,
                    key.section,
                    &options,
                    &geometry.mineral_growth,
                    &remaining_segments,
                    folded,
                    None,
                )?
            };
            // The core and chips are closed even where their original neighbors hid faces.
            // Exact cell-edge faces seal neighbors; relief and material textures remain
            // on the untouched section, and all six real material slots stay intact.
            if let Some(template) = templates.blocks.get_mut(&key.material) {
                template.faces =
                    crate::blocks::cube_faces(0.0, template.face_materials, template.side_cap);
                template.surface = None;
                template.bevel_width = 0.0;
                template.details.clear();
            }
            if !terra_world::segments::is_segment(key.material) {
                section.extend(mining_geometry::cavity_faces(
                    key.position,
                    key.material,
                    key.neighbors,
                    neighbor_tints,
                    &templates,
                    &options,
                )?);
            }
            let mut block = compose_section_with_leaf_detail(
                &mining_geometry::full_faces(cell, key.material),
                &templates,
                key.section,
                &options,
                &geometry.mineral_growth,
                &selected_segments,
                folded,
                None,
            )?;
            if matches!(key.material, 30 | 37) || terra_world::segments::is_segment(key.material) {
                return Ok(Geometry {
                    section,
                    block: Vec::new(),
                    rim: Vec::new(),
                    model_pieces: super::model_pieces::partition(block)?,
                });
            }
            let local_center = Vec3::from_array(cell.map(|v| v as f32 + 0.5));
            for vertex in block.iter_mut().flat_map(|batch| &mut batch.positions) {
                *vertex = (Vec3::from_array(*vertex) - local_center).to_array();
            }
            let rim = mining_geometry::rim(&block);
            Ok(Geometry {
                section,
                block,
                rim,
                model_pieces: Vec::new(),
            })
        }),
    ));
}

fn piece_layout(index: usize) -> (Vec3, f32, usize) {
    if index == 8 {
        return (Vec3::ZERO, 0.86, usize::MAX);
    }
    (
        Vec3::new(
            if index & 1 == 0 { -0.25 } else { 0.25 },
            if index & 2 == 0 { -0.25 } else { 0.25 },
            if index & 4 == 0 { -0.25 } else { 0.25 },
        ),
        0.496,
        (index * 5 + 3) % 8,
    )
}

fn spawn_batch(
    commands: &mut Commands,
    world: &BuildingWorld,
    scene: &Scene,
    root: Entity,
    slot: usize,
    mesh: Handle<Mesh>,
    transform: Transform,
) -> Entity {
    let mut entity = commands.spawn((
        Mesh3d(mesh),
        transform,
        Visibility::default(),
        ChildOf(root),
    ));
    if let Some(water) = scene.water.as_ref().and_then(|water| {
        world
            .blocks
            .templates
            .as_ref()
            .and_then(|t| water.for_slot(t, slot))
    }) {
        entity.insert((MeshMaterial3d(water), bevy::light::NotShadowCaster));
    } else if let Some(foliage) = scene.foliage.as_ref().and_then(|a| a.materials.get(&slot)) {
        entity.insert(MeshMaterial3d(foliage.clone()));
    } else if let Some(material) = world.blocks.materials.get(slot) {
        entity.insert(MeshMaterial3d(material.clone()));
    }
    if world
        .blocks
        .templates
        .as_ref()
        .is_some_and(|t| t.glass_material(slot))
    {
        entity.insert(bevy::light::NotShadowCaster);
    }
    entity.id()
}

pub(super) fn animate(
    world: BuildingWorld,
    mut pieces: Query<(&MiningPiece, &mut Transform, &mut Visibility)>,
) {
    let removed = ((world.mining.progress - 0.15).max(0.0) * 10.0).floor() as usize;
    let strength = 0.002 + world.mining.progress.clamp(0.0, 1.0) * 0.003;
    for (piece, mut transform, mut visibility) in &mut pieces {
        if piece.model {
            let age = (world.mining.progress - (0.15 + piece.order as f32 * 0.085)).max(0.0);
            *visibility = if age > 0.14 {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
            let phase =
                world.time.elapsed_secs() * 34.0 + piece.offset.dot(Vec3::new(7.0, 13.0, 23.0));
            let direction = Vec3::new(piece.offset.x, 0.15, piece.offset.z).normalize_or_zero();
            transform.translation = piece.base
                + Vec3::new(phase.sin(), (phase * 1.3).cos(), (phase * 0.7).sin()) * strength
                + direction * age * 1.7
                - Vec3::Y * age * age * 12.0;
            transform.rotation = Quat::from_euler(EulerRot::XYZ, age * 2.0, age * 3.0, age);
            transform.scale = Vec3::splat((1.0 - age * 3.0).max(0.0));
            continue;
        }
        *visibility = if piece.order < removed {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        let phase = world.time.elapsed_secs() * 34.0 + piece.offset.dot(Vec3::new(7.0, 13.0, 23.0));
        transform.translation = piece.base
            + piece.offset
            + Vec3::new(phase.sin(), (phase * 1.3).cos(), (phase * 0.7).sin()) * strength;
        transform.scale = Vec3::splat(piece.scale);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_workbench_isolates_from_secondary_across_section_border_without_cube_overlay() {
        let mut app = super::super::tests::app();
        let position = [15, 8, 8];
        let peer = [16, 8, 8];
        let section = terra_voxel::blocks::global_to_section(position).0;
        let original = app.world_mut().spawn(Visibility::Inherited).id();
        let material = app
            .world()
            .resource::<super::super::BuildingAssets>()
            .fallback
            .clone();
        let mut primitive_batches = std::collections::BTreeMap::new();
        for x in [0.15, 0.85] {
            crate::props::add_box(
                &mut primitive_batches,
                0,
                Vec3::new(x, 0.4, 0.5),
                Vec3::new(0.1, 0.8, 0.8),
                Quat::IDENTITY,
            )
            .unwrap();
        }
        let mut templates = crate::blocks::TemplateSet::default();
        templates.wood_materials.insert(0);
        templates.blocks.insert(
            28,
            crate::blocks::BlockTemplate {
                surface: None,
                render_kind: crate::blocks::definitions::BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model: primitive_batches
                    .into_values()
                    .map(|batch| crate::blocks::PrimitiveTemplate {
                        tangents: batch.tangents,
                        positions: batch.positions,
                        normals: batch.normals,
                        uvs: batch.uvs,
                        indices: batch.indices,
                        material: batch.material,
                    })
                    .collect(),
            },
        );
        let mut journal = EditJournal::default();
        for (cell, other, secondary) in [(position, peer, false), (peer, position, true)] {
            journal.record(cell, 28).unwrap();
            journal
                .set_object_state(
                    cell,
                    Some(terra_world::edits::BlockObjectState {
                        peer: Some(other),
                        secondary,
                        wood: terra_core::WoodSpecies::Birch,
                        ..default()
                    }),
                )
                .unwrap();
            let (owner, local) = terra_voxel::blocks::global_to_section(cell);
            let mut chunks = app
                .world_mut()
                .resource_mut::<terra_world::streaming::ChunkStore>();
            let loaded = chunks.loaded.entry(owner).or_insert_with(|| {
                terra_world::streaming::LoadedSection {
                    data: terra_voxel::BlockSection::empty(owner).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                }
            });
            loaded.data.set_block(local, 28).unwrap();
        }
        let epoch = journal.mesh_epoch(section);
        let appearance = journal.appearance_epoch(section);
        let snapshot = crate::meshing::custom::capture(
            &app.world()
                .resource::<terra_world::streaming::ChunkStore>()
                .loaded[&section]
                .data,
            &templates,
            &journal,
            |_| Some(0),
        )
        .unwrap();
        let mut cache = GeometryCache::default();
        cache.insert_model_test(
            section,
            epoch,
            terra_voxel::meshing::BlockMesh::default(),
            snapshot,
        );
        app.insert_resource(cache).insert_resource(journal);
        let mut rendered = RenderedSections::default();
        rendered.sections.insert(
            section,
            crate::meshing::RenderedSection {
                entity: original,
                meshes: Vec::new(),
                mesh_epoch: epoch,
                appearance_epoch: appearance,
                detail_level: DetailLevel::Near,
                folded_leaves: None,
                micro_detail: true,
            },
        );
        app.insert_resource(rendered);
        {
            let mut blocks = app.world_mut().resource_mut::<crate::blocks::BlockAssets>();
            blocks.templates = Some(std::sync::Arc::new(templates));
            blocks.materials.push(material);
        }
        app.world_mut()
            .resource_mut::<terra_player::CameraInputState>()
            .captured = true;
        {
            let mut mining = app
                .world_mut()
                .resource_mut::<terra_player::interaction::MiningVisual>();
            mining.target = Some(MiningTarget::Block {
                position: peer,
                expected: 28,
            });
            mining.progress = 0.05;
        }
        for _ in 0..100 {
            app.update();
            if app.world().resource::<Isolation>().active.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let active = app.world().resource::<Isolation>().active.as_ref().unwrap();
        assert_eq!(active.key.owner, position);
        assert_eq!(active.key.position, peer);
        assert!(
            app.world()
                .resource::<super::super::BuildingStatus>()
                .isolated_block
        );
        assert_eq!(
            *app.world().get::<Visibility>(original).unwrap(),
            Visibility::Hidden
        );
        let pieces: Vec<_> = app
            .world_mut()
            .query::<(&MiningPiece, &Mesh3d)>()
            .iter(app.world())
            .map(|(piece, mesh)| (piece.base, piece.model, mesh.0.clone()))
            .collect();
        assert!(!pieces.is_empty() && pieces.len() <= 8);
        let mut minimum = Vec3::splat(f32::INFINITY);
        let mut maximum = Vec3::splat(f32::NEG_INFINITY);
        for (base, model, handle) in pieces {
            assert!(model);
            let mesh = app.world().resource::<Assets<Mesh>>().get(&handle).unwrap();
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(points)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("Missing imported model positions")
            };
            for point in points {
                let point = Vec3::from_array(*point) + base;
                minimum = minimum.min(point);
                maximum = maximum.max(point);
                assert!(
                    point.x < 15.41 || point.x > 16.59,
                    "mining replaced model gap with a cube"
                );
            }
            let Some(bevy::mesh::VertexAttributeValues::Float32x4(colors)) =
                mesh.attribute(Mesh::ATTRIBUTE_COLOR)
            else {
                panic!("Missing model wood tint")
            };
            assert!(
                colors
                    .iter()
                    .all(|&color| color == terra_core::WoodSpecies::Birch.tint())
            );
        }
        assert!((minimum.x - 15.2).abs() < 1.0e-4);
        assert!((maximum.x - 16.8).abs() < 1.0e-4);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<super::super::fracture::FractureOverlay>>()
                .iter(app.world())
                .count(),
            0
        );
        app.world_mut()
            .resource_mut::<terra_player::interaction::MiningVisual>()
            .target = None;
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(original).unwrap(),
            Visibility::Inherited
        );
        assert_eq!(
            app.world_mut()
                .query::<&MiningPiece>()
                .iter(app.world())
                .count(),
            0
        );
    }

    #[test]
    fn asynchronous_replacement_restores_original_on_cancel_and_target_change() {
        let mut app = super::super::tests::app();
        let section = SectionPos::default();
        let position = [8, 8, 8];
        let original = app.world_mut().spawn(Visibility::Inherited).id();
        let material = app
            .world()
            .resource::<super::super::BuildingAssets>()
            .fallback
            .clone();
        let mut templates = crate::blocks::TemplateSet::default();
        templates.blocks.insert(
            3,
            crate::blocks::BlockTemplate {
                surface: None,
                render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
                bevel_width: 0.0,
                faces: crate::blocks::cube_faces(0.0, [0; 6], None),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model: Vec::new(),
            },
        );
        {
            let mut blocks = app.world_mut().resource_mut::<crate::blocks::BlockAssets>();
            blocks.templates = Some(std::sync::Arc::new(templates));
            blocks.materials.push(material);
        }
        let mut cache = GeometryCache::default();
        cache.insert_test(
            section,
            0,
            terra_voxel::meshing::BlockMesh {
                faces: mining_geometry::full_faces([8; 3], 3),
                ..default()
            },
        );
        app.insert_resource(cache).init_resource::<EditJournal>();
        let mut rendered = RenderedSections::default();
        rendered.sections.insert(
            section,
            crate::meshing::RenderedSection {
                entity: original,
                meshes: Vec::new(),
                mesh_epoch: 0,
                appearance_epoch: 0,
                detail_level: DetailLevel::Near,
                folded_leaves: None,
                micro_detail: true,
            },
        );
        app.insert_resource(rendered);
        app.world_mut()
            .resource_mut::<terra_world::streaming::ChunkStore>()
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block([8; 3], 3)
            .unwrap();
        app.world_mut()
            .resource_mut::<terra_player::CameraInputState>()
            .captured = true;
        {
            let mut mining = app
                .world_mut()
                .resource_mut::<terra_player::interaction::MiningVisual>();
            mining.target = Some(MiningTarget::Block {
                position,
                expected: 3,
            });
            mining.progress = 0.3;
        }
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(original).unwrap(),
            Visibility::Inherited
        );
        for _ in 0..100 {
            app.update();
            if app.world().resource::<Isolation>().active.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(app.world().resource::<Isolation>().active.is_some());
        assert_eq!(
            *app.world().get::<Visibility>(original).unwrap(),
            Visibility::Hidden
        );
        assert_eq!(
            app.world()
                .resource::<terra_world::streaming::ChunkStore>()
                .block_at(position),
            Some(3)
        );
        assert_eq!(app.world().resource::<EditJournal>().mesh_epoch(section), 0);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<MiningPiece>>()
                .iter(app.world())
                .count(),
            9
        );
        app.world_mut()
            .resource_mut::<terra_player::interaction::MiningVisual>()
            .target = None;
        app.update();
        assert!(app.world().resource::<Isolation>().active.is_none());
        assert_eq!(
            *app.world().get::<Visibility>(original).unwrap(),
            Visibility::Inherited
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<MiningPiece>>()
                .iter(app.world())
                .count(),
            0
        );
        {
            let mut mining = app
                .world_mut()
                .resource_mut::<terra_player::interaction::MiningVisual>();
            mining.target = Some(MiningTarget::Block {
                position,
                expected: 3,
            });
            mining.progress = 0.5;
        }
        app.update();
        app.world_mut()
            .resource_mut::<terra_player::interaction::MiningVisual>()
            .target = Some(MiningTarget::Block {
            position: [7; 3],
            expected: 3,
        });
        app.update();
        assert!(app.world().resource::<Isolation>().pending.is_none());
        assert_eq!(
            *app.world().get::<Visibility>(original).unwrap(),
            Visibility::Inherited
        );
        app.world_mut()
            .resource_mut::<terra_player::interaction::MiningVisual>()
            .target = Some(MiningTarget::Block {
            position,
            expected: 3,
        });
        app.update();
        app.world_mut()
            .resource_mut::<RenderedSections>()
            .sections
            .get_mut(&section)
            .unwrap()
            .mesh_epoch = 1;
        app.update();
        assert!(
            app.world().resource::<Isolation>().pending.is_none(),
            "stale epoch must never upload"
        );
        assert_eq!(
            *app.world().get::<Visibility>(original).unwrap(),
            Visibility::Inherited
        );
        app.world_mut()
            .resource_mut::<RenderedSections>()
            .sections
            .get_mut(&section)
            .unwrap()
            .mesh_epoch = 0;
        for _ in 0..100 {
            app.update();
            if app.world().resource::<Isolation>().active.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(app.world().resource::<Isolation>().active.is_some());
        app.world_mut()
            .resource_mut::<terra_world::streaming::ChunkStore>()
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block([8; 3], 0)
            .unwrap();
        app.world_mut()
            .resource_mut::<terra_player::interaction::MiningVisual>()
            .target = None;
        for _ in 0..5 {
            app.update();
        }
        assert!(
            app.world()
                .resource::<Isolation>()
                .active
                .as_ref()
                .unwrap()
                .target_root
                .is_none()
        );
        assert_eq!(
            *app.world().get::<Visibility>(original).unwrap(),
            Visibility::Hidden,
            "committed block must never reappear before its ordinary section uploads"
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<MiningPiece>>()
                .iter(app.world())
                .count(),
            0
        );
        let uploaded = app.world_mut().spawn(Visibility::Inherited).id();
        app.world_mut()
            .resource_mut::<RenderedSections>()
            .sections
            .get_mut(&section)
            .unwrap()
            .entity = uploaded;
        app.world_mut().despawn(original);
        app.update();
        assert!(app.world().resource::<Isolation>().active.is_none());
        assert_eq!(
            *app.world().get::<Visibility>(uploaded).unwrap(),
            Visibility::Inherited
        );
        app.world_mut()
            .resource_mut::<terra_world::streaming::ChunkStore>()
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block([8; 3], 3)
            .unwrap();
        app.world_mut()
            .resource_mut::<terra_player::interaction::MiningVisual>()
            .target = Some(MiningTarget::Block {
            position,
            expected: 3,
        });
        for _ in 0..100 {
            app.update();
            if app.world().resource::<Isolation>().active.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(app.world().resource::<Isolation>().active.is_some());
        app.world_mut()
            .resource_mut::<terra_world::streaming::ChunkStore>()
            .loaded
            .remove(&section);
        app.world_mut().despawn(uploaded);
        app.update();
        assert!(app.world().resource::<Isolation>().active.is_none());
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<MiningPiece>>()
                .iter(app.world())
                .count(),
            0
        );
    }
}
