use super::resources::*;
use crate::{
    edits::EditJournal,
    water::{FluidOccluders, FluidSnapshot, WaterPatch, append_fluid_patch},
};
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::{collections::HashMap, sync::Arc};
use terra_core::{block_properties::BlockPropertiesCatalog, block_shapes::ShapeBox};
use terra_voxel::SectionPos;

struct WaterOutput {
    epoch: u64,
    patch: Option<WaterPatch>,
}
#[derive(Resource, Default)]
pub(super) struct PendingWaterJobs(HashMap<SectionPos, Task<WaterOutput>>);

pub(super) fn dispatch_water_jobs(
    settings: Res<StreamingSettings>,
    terrain: Res<WorldTerrain>,
    journal: Res<EditJournal>,
    properties: Res<BlockPropertiesCatalog>,
    mut chunks: ResMut<ChunkStore>,
    mut pending: ResMut<PendingWaterJobs>,
) {
    if !settings.meshes || !terrain.0.config().water {
        return;
    }
    let refresh = journal.is_changed() || chunks.is_changed();
    // These queues are worker bookkeeping, not observable world changes. Only
    // publishing a completed patch may wake render caches through ChunkStore.
    let chunks = chunks.bypass_change_detection();
    if refresh {
        pending.0.retain(|p, _| chunks.loaded.contains_key(p));
        for (p, epoch) in journal.water_mesh_epochs() {
            if chunks
                .loaded
                .get(&p)
                .is_some_and(|section| section.water_epoch != epoch)
            {
                chunks.water_dirty.insert(p);
            }
        }
        chunks
            .water_dirty
            .retain(|p| chunks.loaded.contains_key(p) && journal.water_mesh_epoch(*p) > 0);
    }
    while pending.0.len() < 2 {
        let next = chunks
            .water_dirty
            .iter()
            .copied()
            .filter(|p| {
                !pending.0.contains_key(p)
                    && chunks
                        .loaded
                        .get(p)
                        .is_some_and(|s| s.mesh_epoch == journal.mesh_epoch(*p))
            })
            .min_by_key(|p| (p.y, p.z, p.x));
        let Some(position) = next else { break };
        chunks.water_dirty.remove(&position);
        let Ok(origin) = position.origin() else {
            continue;
        };
        let Some(column) = chunks.columns.get(&[position.x, position.z]).cloned() else {
            continue;
        };
        let Ok(mut fluids) = journal.fluid_snapshot(position) else {
            continue;
        };
        let Ok(overrides) = journal.snapshot(position) else {
            continue;
        };
        let mut padded = vec![u16::MAX; 18 * 18 * 18];
        for z in 0..18 {
            for y in 0..18 {
                for x in 0..18 {
                    let global = [
                        origin[0] + x as i64 - 1,
                        origin[1] + y as i64 - 1,
                        origin[2] + z as i64 - 1,
                    ];
                    if let Some(block) = overrides
                        .get(&global)
                        .copied()
                        .or_else(|| chunks.block_at(global))
                    {
                        padded[x + 18 * (y + 18 * z)] = block;
                        if crate::water::space::partial(block) && !fluids.contains_key(&global) {
                            let fluid = chunks.fluid_cell_at(&terrain.0, &journal, global, block);
                            if fluid.amount > 0 {
                                fluids.insert(global, fluid);
                            }
                        }
                    }
                }
            }
        }
        let occluders = capture_occluders(chunks, &journal, &properties, origin, &fluids);
        let generator = crate::SectionGenerator::new(terrain.0.clone());
        let epoch = journal.water_mesh_epoch(position);
        pending.0.insert(
            position,
            AsyncComputeTaskPool::get().spawn(async move {
                // Boundary sections may not yet be loaded. This fallback runs only
                // on the worker and respects captured block edits above.
                for z in 0..18 {
                    for y in 0..18 {
                        for x in 0..18 {
                            let index = x + 18 * (y + 18 * z);
                            if padded[index] == u16::MAX {
                                let global = [
                                    origin[0] + x as i64 - 1,
                                    origin[1] + y as i64 - 1,
                                    origin[2] + z as i64 - 1,
                                ];
                                padded[index] = generator.block_at(global).unwrap_or(u16::MAX);
                            }
                        }
                    }
                }
                WaterOutput {
                    epoch,
                    patch: build_patch(
                        &generator, position, origin, column, &padded, &fluids, &occluders,
                    ),
                }
            }),
        );
    }
}

fn capture_occluders(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    origin: [i64; 3],
    fluids: &FluidSnapshot,
) -> FluidOccluders {
    let mut occluders = FluidOccluders::new();
    let block_at = |p| journal.block(p).or_else(|| chunks.block_at(p));
    for (&p, cell) in fluids {
        if cell.amount == 0 || !(0..3).all(|i| (origin[i]..origin[i] + 16).contains(&p[i])) {
            continue;
        }
        // Shared top corners sample all four X/Z cells and their connection
        // above. Cardinal-only masks omit diagonal waterlogged construction.
        for q in (-1..=1).flat_map(|z| {
            (-1..=1).flat_map(move |y| {
                (-1..=1).filter_map(move |x| crate::water::fluid::adjacent(p, [x, y, z]))
            })
        }) {
            if block_at(q).is_none_or(|id| {
                crate::water::can_hold_fluid(id) && !crate::water::space::partial(id)
            }) || occluders.contains_key(&q)
            {
                continue;
            }
            let mut boxes = Vec::new();
            if crate::interaction_shapes::visit_boxes_with(journal, properties, q, block_at, |b| {
                boxes.push(b)
            }) {
                occluders.insert(q, boxes);
            } else if block_at(q).is_some_and(|material| {
                terra_voxel::MaterialId::from_raw(material).is_some_and(|m| m.is_solid())
            }) {
                occluders.insert(
                    q,
                    vec![ShapeBox {
                        minimum: q.map(|v| v as f64),
                        maximum: q.map(|v| v as f64 + 1.0),
                        hinged: false,
                    }],
                );
            }
        }
    }
    occluders
}

fn build_patch(
    generator: &crate::SectionGenerator,
    position: SectionPos,
    origin: [i64; 3],
    column: Arc<crate::PreparedColumn>,
    padded: &[u16],
    fluids: &FluidSnapshot,
    occluders: &FluidOccluders,
) -> Option<WaterPatch> {
    let columns = column.water_columns();
    let config = generator.terrain().config();
    let local_levels = config.rivers || crate::water::local_water_levels(columns, config.sea_level);
    // Explicit liquid cells own their surface, including an empty source override.
    // Do not draw the implicit reservoir at the exact same depth as that cell.
    let mut natural = padded.to_vec();
    for p in fluids.keys() {
        let local = std::array::from_fn::<_, 3, _>(|i| p[i] - origin[i] + 1);
        if local.iter().all(|v| (0..18).contains(v)) {
            let [x, y, z] = local.map(|v| v as usize);
            natural[x + 18 * (y + 18 * z)] = u16::MAX;
            // Integer-height reservoirs assign their top to the cell above,
            // while the simulated volume (and its top) belongs to the cell below.
            if y + 1 < 18
                && columns[x + 18 * z]
                    .water_columns()
                    .intervals
                    .into_iter()
                    .flatten()
                    .any(|w| (w.level - (p[1] + 1) as f64).abs() < 1e-8)
            {
                natural[x + 18 * (y + 1 + 18 * z)] = u16::MAX;
            }
        }
    }
    let mut patch = crate::water::build_natural_patch(
        position,
        origin,
        columns,
        &natural,
        config.sea_level,
        local_levels,
    );
    append_fluid_patch(
        &mut patch,
        position,
        columns,
        padded,
        fluids,
        (local_levels, occluders),
    );
    patch
}

pub(super) fn receive_water_jobs(
    journal: Res<EditJournal>,
    mut chunks: ResMut<ChunkStore>,
    mut pending: ResMut<PendingWaterJobs>,
    mut published: MessageWriter<WaterPublished>,
) {
    let mut finished = Vec::new();
    for (&p, task) in &mut pending.0 {
        if let Some(output) = future::block_on(future::poll_once(task)) {
            finished.push((p, output));
        }
    }
    for (p, output) in finished {
        pending.0.remove(&p);
        if publish_water_patch(&journal, chunks.bypass_change_detection(), p, output) {
            chunks.set_changed();
            published.write(WaterPublished { position: p });
        }
    }
}

fn publish_water_patch(
    journal: &EditJournal,
    chunks: &mut ChunkStore,
    p: SectionPos,
    output: WaterOutput,
) -> bool {
    let Some(section) = chunks.loaded.get(&p) else {
        return false;
    };
    if output.epoch != journal.water_mesh_epoch(p) || section.mesh_epoch != journal.mesh_epoch(p) {
        chunks.water_dirty.insert(p);
        return false;
    }
    let _scope = terra_core::profiling::ScopeTimer::new("streaming.publish_water_patch");
    let Some(section) = chunks.loaded.get_mut(&p) else {
        return false;
    };
    section.water = output.patch;
    section.water_epoch = output.epoch;
    chunks.water_dirty.remove(&p);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::WorldConfig;

    #[test]
    fn explicit_cells_replace_reservoir_tops_instead_of_z_fighting_at_integral_levels() {
        let terrain = crate::TerraGenerator::new(WorldConfig {
            base_height: 3.,
            sea_level: 8.,
            hill_amplitude: 0.,
            mountain_amplitude: 0.,
            biomes: false,
            caves: false,
            trees: false,
            rivers: false,
            ..Default::default()
        })
        .unwrap();
        let generator = crate::SectionGenerator::new(terrain);
        let position = SectionPos::default();
        let column = Arc::new(generator.prepare_column([0, 0], true).unwrap());
        let padded = vec![0; 18 * 18 * 18];
        for (cell, expected) in [
            (
                crate::water::FluidCell {
                    amount: 8,
                    source: true,
                    flow: [0; 2],
                },
                512,
            ),
            (crate::water::FluidCell::default(), 510),
        ] {
            let fluids = FluidSnapshot::from([([8, 7, 8], cell)]);
            let patch = build_patch(
                &generator,
                position,
                [0; 3],
                column.clone(),
                &padded,
                &fluids,
                &FluidOccluders::new(),
            )
            .unwrap();
            let tops = patch
                .indices
                .chunks_exact(3)
                .filter(|triangle| {
                    triangle.iter().all(|&i| {
                        patch
                            .normals
                            .get(i as usize)
                            .copied()
                            .unwrap_or([0., 1., 0.])[1]
                            > 0.99
                            && (patch.positions[i as usize][1] - 8.).abs() < 0.001
                    })
                })
                .count();
            assert_eq!(
                tops, expected,
                "explicit source/dry surface must have exactly one owner"
            );
        }
    }

    #[derive(Resource, Default)]
    struct ChangeObservations(usize);

    #[test]
    fn idle_water_jobs_do_not_mark_world_or_journal_changed() {
        let mut app = App::new();
        app.init_resource::<ChunkStore>()
            .add_message::<WaterPublished>()
            .init_resource::<EditJournal>()
            .init_resource::<BlockPropertiesCatalog>()
            .init_resource::<PendingWaterJobs>()
            .init_resource::<ChangeObservations>()
            .insert_resource(StreamingSettings {
                radius: 1,
                vertical_radius: 1,
                max_jobs: 1,
                meshes: true,
            })
            .insert_resource(WorldTerrain(
                crate::TerraGenerator::new(WorldConfig::default()).unwrap(),
            ))
            .add_systems(
                Update,
                (receive_water_jobs, dispatch_water_jobs, observe_changes).chain(),
            );
        app.update();
        assert_eq!(app.world().resource::<ChangeObservations>().0, 1);
        for _ in 0..8 {
            app.update();
        }
        assert_eq!(app.world().resource::<ChangeObservations>().0, 1);
    }

    fn observe_changes(
        chunks: Res<ChunkStore>,
        journal: Res<EditJournal>,
        mut observations: ResMut<ChangeObservations>,
    ) {
        if chunks.is_changed() || journal.is_changed() {
            observations.0 += 1;
        }
    }

    #[test]
    fn fluid_occluders_use_current_neighbor_rotations_and_replaced_blocks() {
        use crate::edits::BlockObjectState;
        use terra_voxel::{BlockSection, global_to_section};

        for owner in [SectionPos::default(), SectionPos::new(-1, -1, -1)] {
            let origin = owner.origin().unwrap();
            let mut chunks = ChunkStore::default();
            for position in [owner, owner.offset([1, 0, 0]).unwrap()] {
                chunks.loaded.insert(
                    position,
                    LoadedSection {
                        data: BlockSection::from_blocks(position, 0, vec![0; 4096]).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
            }
            let p = [origin[0] + 15, origin[1] + 7, origin[2] + 8];
            let q = [p[0] + 1, p[1], p[2]];
            let (neighbor, local) = global_to_section(q);
            chunks
                .loaded
                .get_mut(&neighbor)
                .unwrap()
                .data
                .set_block(local, 82)
                .unwrap();
            let mut journal = EditJournal::default();
            journal.record(q, 82).unwrap();
            let properties = BlockPropertiesCatalog::default();
            let fluids = FluidSnapshot::from([(
                p,
                crate::water::FluidCell {
                    amount: 8,
                    ..Default::default()
                },
            )]);
            for facing in [0, 4, 5, 6, 9, 10] {
                journal
                    .set_object_state(
                        q,
                        Some(BlockObjectState {
                            facing,
                            ..Default::default()
                        }),
                    )
                    .unwrap();
                let mut boxes = Vec::new();
                assert!(crate::interaction_shapes::visit_boxes(
                    &chunks,
                    &journal,
                    &properties,
                    q,
                    |b| boxes.push(b)
                ));
                assert_eq!(
                    capture_occluders(&chunks, &journal, &properties, origin, &fluids).get(&q),
                    Some(&boxes)
                );
            }
            let loaded = capture_occluders(&chunks, &journal, &properties, origin, &fluids);
            let saved = chunks.loaded.remove(&neighbor).unwrap();
            assert_eq!(
                capture_occluders(&chunks, &journal, &properties, origin, &fluids),
                loaded
            );
            let before = journal.water_mesh_epoch(owner);
            journal
                .set_object_state(
                    q,
                    Some(BlockObjectState {
                        facing: 4,
                        ..Default::default()
                    }),
                )
                .unwrap();
            assert!(
                journal.water_mesh_epoch(owner) > before,
                "neighbor rotation must reject an in-flight water patch across the section edge"
            );
            let unloaded = capture_occluders(&chunks, &journal, &properties, origin, &fluids);
            assert_ne!(unloaded, loaded);
            chunks.loaded.insert(neighbor, saved);
            assert_eq!(
                capture_occluders(&chunks, &journal, &properties, origin, &fluids),
                unloaded,
                "loading the persisted neighbor must not change the fluid occlusion"
            );
            chunks
                .loaded
                .get_mut(&neighbor)
                .unwrap()
                .data
                .set_block(local, 3)
                .unwrap();
            journal.record(q, 3).unwrap();
            assert_eq!(
                capture_occluders(&chunks, &journal, &properties, origin, &fluids).get(&q),
                Some(&vec![ShapeBox {
                    minimum: q.map(|v| v as f64),
                    maximum: q.map(|v| v as f64 + 1.0),
                    hinged: false,
                }]),
                "replacing a partial block with a full solid must retain its complete fluid occlusion"
            );
            chunks
                .loaded
                .get_mut(&neighbor)
                .unwrap()
                .data
                .set_block(local, 0)
                .unwrap();
            journal.record(q, 0).unwrap();
            assert!(capture_occluders(&chunks, &journal, &properties, origin, &fluids).is_empty());
        }
    }

    #[test]
    fn diagonal_waterlogged_masks_keep_a_shared_corner_watertight_across_four_chunks() {
        use crate::water::FluidCell;
        use terra_voxel::{BlockSection, global_to_section};

        let terrain = crate::TerraGenerator::new(WorldConfig {
            base_height: -20.0,
            sea_level: -30.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            caves: false,
            biomes: false,
            trees: false,
            rivers: false,
            ..Default::default()
        })
        .unwrap();
        let columns = vec![terrain.surface_column(0.5, 0.5); 18 * 18];
        let properties = BlockPropertiesCatalog::default();
        for base in [SectionPos::default(), SectionPos::new(-1, -1, -1)] {
            let origin = base.origin().unwrap();
            let corner = [origin[0] + 16, origin[1] + 7, origin[2] + 16];
            let mut chunks = ChunkStore::default();
            let mut journal = EditJournal::default();
            for z in 0..2 {
                for x in 0..2 {
                    let position = base.offset([x, 0, z]).unwrap();
                    chunks.loaded.insert(
                        position,
                        LoadedSection {
                            data: BlockSection::from_blocks(position, 0, vec![0; 4096]).unwrap(),
                            mesh: None,
                            mesh_epoch: 0,
                            water_epoch: 0,
                            water: None,
                            props: Vec::new(),
                        },
                    );
                    journal
                        .set_fluid(
                            [corner[0] - 1 + x, corner[1], corner[2] - 1 + z],
                            FluidCell {
                                amount: if x == 1 && z == 1 { 6 } else { 1 },
                                ..Default::default()
                            },
                        )
                        .unwrap();
                }
            }
            let slab = corner;
            let (owner, local) = global_to_section(slab);
            chunks
                .loaded
                .get_mut(&owner)
                .unwrap()
                .data
                .set_block(local, 82)
                .unwrap();
            journal.record(slab, 82).unwrap();
            for z in 0..2 {
                for x in 0..2 {
                    let position = base.offset([x, 0, z]).unwrap();
                    let origin = position.origin().unwrap();
                    let fluids = journal.fluid_snapshot(position).unwrap();
                    let occluders =
                        capture_occluders(&chunks, &journal, &properties, origin, &fluids);
                    assert!(
                        occluders.contains_key(&slab),
                        "the diagonal mask belongs to every corner owner"
                    );
                    let mut padded = vec![0; 18 * 18 * 18];
                    let local: [usize; 3] =
                        std::array::from_fn(|i| (slab[i] - origin[i] + 1) as usize);
                    padded[local[0] + 18 * (local[1] + 18 * local[2])] = 82;
                    let mut patch = None;
                    append_fluid_patch(
                        &mut patch,
                        position,
                        &columns,
                        &padded,
                        &fluids,
                        (false, &occluders),
                    );
                    let patch = patch.unwrap();
                    let heights: Vec<_> = patch
                        .positions
                        .iter()
                        .zip(&patch.normals)
                        .filter(|(point, normal)| {
                            normal[1] > 0.5
                                && point[0] + origin[0] as f32 == corner[0] as f32
                                && point[2] + origin[2] as f32 == corner[2] as f32
                        })
                        .map(|(point, _)| point[1] + origin[1] as f32)
                        .collect();
                    assert!(!heights.is_empty());
                    assert!(
                        heights
                            .iter()
                            .all(|height| *height == corner[1] as f32 + 0.75),
                        "{position:?}: {heights:?}"
                    );
                }
            }

            let upper = [slab[0], slab[1] + 1, slab[2]];
            journal.record(upper, 82).unwrap();
            journal
                .set_object_state(
                    upper,
                    Some(crate::edits::BlockObjectState {
                        facing: 4,
                        ..Default::default()
                    }),
                )
                .unwrap();
            journal
                .set_fluid(
                    upper,
                    FluidCell {
                        amount: 8,
                        ..Default::default()
                    },
                )
                .unwrap();
            let fluids = journal.fluid_snapshot(base).unwrap();
            let masks = capture_occluders(&chunks, &journal, &properties, origin, &fluids);
            assert!(
                masks.contains_key(&upper),
                "diagonal connection-above samples also require the authored mask"
            );
        }
    }

    #[test]
    fn second_neighbor_shape_changes_invalidate_published_and_inflight_water_without_widening_fluid_ticks()
     {
        use crate::edits::BlockObjectState;
        use terra_voxel::BlockSection;

        for owner in [SectionPos::default(), SectionPos::new(-1, -1, -1)] {
            let origin = owner.origin().unwrap();
            let mut chunks = ChunkStore::default();
            for position in [owner, owner.offset([1, 0, 0]).unwrap()] {
                chunks.loaded.insert(
                    position,
                    LoadedSection {
                        data: BlockSection::from_blocks(position, 0, vec![0; 4096]).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
            }
            let p = [origin[0] + 15, origin[1] + 7, origin[2] + 8];
            let q = [p[0] + 1, p[1], p[2]];
            let r = [p[0] + 2, p[1], p[2]];
            let mut journal = EditJournal::default();
            journal.record(q, 74).unwrap();
            let cell = crate::water::FluidCell {
                amount: 8,
                ..Default::default()
            };
            journal.set_fluid(p, cell).unwrap();
            let properties = BlockPropertiesCatalog::default();
            let fluids = FluidSnapshot::from([(p, cell)]);
            let before_boxes = capture_occluders(&chunks, &journal, &properties, origin, &fluids);
            assert!(before_boxes.contains_key(&q));
            chunks.loaded.get_mut(&owner).unwrap().mesh_epoch = journal.mesh_epoch(owner);
            let previous_mesh_epoch = journal.mesh_epoch(owner);
            let captured_epoch = journal.water_mesh_epoch(owner);
            let patch = |height| {
                Some(WaterPatch {
                    positions: vec![
                        [0.0, height, 0.0],
                        [0.0, height, 1.0],
                        [1.0, height, 1.0],
                        [1.0, height, 0.0],
                    ],
                    normals: vec![[0.0, 1.0, 0.0]; 4],
                    flow: vec![[0.0; 2]; 4],
                    depth_shore: vec![[1.0, 0.0]; 4],
                    surface_effects: vec![[0.0; 4]; 4],
                    wave_weights: vec![0.0; 4],
                    indices: vec![0, 1, 2, 0, 2, 3],
                })
            };
            publish_water_patch(
                &journal,
                &mut chunks,
                owner,
                WaterOutput {
                    epoch: captured_epoch,
                    patch: patch(1.0),
                },
            );
            let inflight = WaterOutput {
                epoch: captured_epoch,
                patch: patch(2.0),
            };

            journal.record(r, 74).unwrap();
            journal
                .set_object_state(
                    r,
                    Some(BlockObjectState {
                        facing: 8,
                        ..Default::default()
                    }),
                )
                .unwrap();
            assert_ne!(
                before_boxes,
                capture_occluders(&chunks, &journal, &properties, origin, &fluids),
                "wall-stair connector at x17 must change the occluding shape at x16"
            );
            assert_eq!(
                journal.mesh_epoch(owner),
                previous_mesh_epoch,
                "the fix must not trigger a terrain mesh rebuild beyond its existing halo"
            );
            assert!(journal.water_mesh_epoch(owner) > captured_epoch);
            assert_ne!(
                chunks.loaded[&owner].water_epoch,
                journal.water_mesh_epoch(owner),
                "the already published patch must become dirty"
            );

            publish_water_patch(&journal, &mut chunks, owner, inflight);
            assert!(chunks.water_dirty.contains(&owner));
            assert_eq!(
                chunks.loaded[&owner].water.as_ref().unwrap().positions[0][1],
                1.0,
                "the stale in-flight patch must not replace the existing mesh"
            );
            let current_epoch = journal.water_mesh_epoch(owner);
            publish_water_patch(
                &journal,
                &mut chunks,
                owner,
                WaterOutput {
                    epoch: current_epoch,
                    patch: patch(3.0),
                },
            );
            assert!(!chunks.water_dirty.contains(&owner));
            assert_eq!(chunks.loaded[&owner].water_epoch, current_epoch);
            assert_eq!(
                chunks.loaded[&owner].water.as_ref().unwrap().positions[0][1],
                3.0
            );

            journal.set_fluid([r[0], r[1] + 1, r[2]], cell).unwrap();
            assert_eq!(
                journal.water_mesh_epoch(owner),
                current_epoch,
                "ordinary fluid updates must retain the narrow one-cell halo"
            );
        }
    }
}
