use super::*;
use crate::{TerraGenerator, edits::BlockObjectState, water::FluidCell};
use terra_core::{WorldConfig, plumbing};
use terra_voxel::{BlockSection, global_to_section};

fn top_area(patch: &WaterPatch) -> f32 {
    patch
        .indices
        .chunks_exact(3)
        .filter_map(|triangle| {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]];
            if patch.normals.get(a as usize).is_some_and(|n| n[1] < 0.99) {
                return None;
            }
            let [a, b, c] = [a, b, c].map(|i| patch.positions[i as usize]);
            Some(((b[0] - a[0]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[0] - a[0])).abs() * 0.5)
        })
        .sum()
}

#[test]
fn lake_surfaces_remain_around_submerged_and_overhead_plumbing_without_duplicate_tops() {
    for level in [8.0, 7.5, 7.3, 16.0, -16.0] {
        let terrain = TerraGenerator::new(WorldConfig {
            base_height: level - 4.,
            sea_level: level,
            hill_amplitude: 0.,
            mountain_amplitude: 0.,
            biomes: false,
            caves: false,
            trees: false,
            rivers: false,
            landscape_revision: 0,
            structures_revision: 0,
            ..Default::default()
        })
        .unwrap();
        let generator = crate::SectionGenerator::new(terrain.clone());
        let coordinate = [-1, -1];
        let column = Arc::new(generator.prepare_column(coordinate, true).unwrap());
        for material in (plumbing::WOOD_PIPE..=plumbing::PUMP).chain([plumbing::GLASS_PIPE]) {
            for facing in 0..if plumbing::is_pipe(material) { 6 } else { 4 } {
                for y in [level.ceil() as i64 - 1, level.ceil() as i64] {
                    for lower_loaded in [true, false] {
                        if !lower_loaded && (level % 16.0 != 0.0 || y as f64 != level) {
                            continue;
                        }
                        let p = [-8, y, -8];
                        let mut chunks = ChunkStore::default();
                        let mut journal = EditJournal::default();
                        let surface = (level.floor() as i64).div_euclid(16);
                        let mut sections = vec![SectionPos::new(-1, surface, -1)];
                        if lower_loaded {
                            sections.push(SectionPos::new(-1, surface - 1, -1));
                        }
                        for &position in &sections {
                            chunks.loaded.insert(
                                position,
                                LoadedSection {
                                    data: BlockSection::empty(position).unwrap(),
                                    mesh: None,
                                    mesh_epoch: 0,
                                    water_epoch: 0,
                                    water: None,
                                    props: vec![],
                                },
                            );
                        }
                        chunks.set_prepared_column_for_test(coordinate, Some(column.clone()));
                        let (owner, local) = global_to_section(p);
                        chunks
                            .loaded
                            .get_mut(&owner)
                            .unwrap()
                            .data
                            .set_block(local, material)
                            .unwrap();
                        journal.record(p, material).unwrap();
                        journal
                            .set_object_state(
                                p,
                                Some(BlockObjectState {
                                    facing,
                                    ..Default::default()
                                }),
                            )
                            .unwrap();
                        // Old versions left dry tombstones where a pipe was put
                        // immediately above an integral natural surface.
                        if y as f64 == level {
                            journal.set_fluid(p, FluidCell::default()).unwrap();
                        }
                        let mut area = 0.;
                        for &position in &sections {
                            let origin = position.origin().unwrap();
                            let mut fluids = journal.fluid_snapshot(position).unwrap();
                            let mut precise_levels = HashMap::new();
                            let mut padded = vec![0; 18 * 18 * 18];
                            let local: [i64; 3] = std::array::from_fn(|i| p[i] - origin[i] + 1);
                            if local.iter().all(|v| (0..18).contains(v)) {
                                let [x, y, z] = local.map(|v| v as usize);
                                padded[x + 18 * (y + 18 * z)] = material;
                                if let Some((cell, level)) = capture_construction_water(
                                    &chunks,
                                    &journal,
                                    &terrain,
                                    column.surface_at(p[0], p[2]).unwrap().water_columns(),
                                    p,
                                    material,
                                    y > 0,
                                    &mut fluids,
                                ) {
                                    precise_levels.insert(cell, level);
                                }
                            }
                            extend_construction_water(
                                &chunks,
                                &journal,
                                &terrain,
                                origin,
                                column.water_columns(),
                                &mut fluids,
                                &mut precise_levels,
                            );
                            let masks = capture_occluders(
                                &chunks,
                                &journal,
                                &BlockPropertiesCatalog::default(),
                                origin,
                                &fluids,
                            );
                            if let Some(patch) = build_patch(
                                &generator,
                                position,
                                origin,
                                column.clone(),
                                &padded,
                                &fluids,
                                &masks,
                                &precise_levels,
                            ) {
                                area += top_area(&patch);
                                for (&point, normal) in patch.positions.iter().zip(&patch.normals) {
                                    if normal[1] > 0.99 {
                                        assert!(
                                            (f64::from(point[1]) + origin[1] as f64 - level).abs()
                                                < 0.00001,
                                            "pipe surface must retain natural height {level}: {point:?}"
                                        );
                                    }
                                }
                                assert!(patch.positions.iter().flatten().all(|v| v.is_finite()));
                                assert!(
                                    patch
                                        .indices
                                        .iter()
                                        .all(|&i| (i as usize) < patch.positions.len())
                                );
                            }
                        }
                        assert!(
                            area > 255.0 && area <= 256.001,
                            "level={level} material={material} facing={facing} y={y} lower_loaded={lower_loaded} area={area}"
                        );
                        if plumbing::is_pipe(material) && facing < 4 && level.fract() == 0.0 {
                            assert!(
                                (area - 256.0).abs() < 0.001,
                                "a pipe clear of the surface cannot remove lake area: {area}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn explicit_full_sources_and_finite_cells_at_natural_y_boundaries_keep_only_the_upper_top() {
    for level in [-16i64, 16] {
        let terrain = TerraGenerator::new(WorldConfig {
            base_height: level as f64 - 4.,
            sea_level: level as f64,
            hill_amplitude: 0.,
            mountain_amplitude: 0.,
            biomes: false,
            caves: false,
            trees: false,
            rivers: false,
            landscape_revision: 0,
            structures_revision: 0,
            ..Default::default()
        })
        .unwrap();
        let generator = crate::SectionGenerator::new(terrain);
        let column = Arc::new(generator.prepare_column([-1, -1], true).unwrap());
        let upper = SectionPos::new(-1, level.div_euclid(16), -1);
        let lower = upper.offset([0, -1, 0]).unwrap();
        let p = [-8, level - 1, -8];
        for source in [true, false] {
            for material in [0, 27] {
                let mut journal = EditJournal::default();
                journal.record(p, material).unwrap();
                journal
                    .set_fluid(
                        p,
                        FluidCell {
                            amount: 8,
                            source,
                            flow: [0; 2],
                        },
                    )
                    .unwrap();
                let mut replica = EditJournal::default();
                replica.import_delta(journal.section_delta(lower)).unwrap();
                let mut areas = [0.; 2];
                for (index, owner) in [lower, upper].into_iter().enumerate() {
                    let origin = owner.origin().unwrap();
                    let mut padded = vec![0; 18 * 18 * 18];
                    let [x, y, z] =
                        std::array::from_fn::<_, 3, _>(|i| (p[i] - origin[i] + 1) as usize);
                    padded[x + 18 * (y + 18 * z)] = material;
                    let fluids = replica.fluid_snapshot(owner).unwrap();
                    assert_eq!(fluids.get(&p).unwrap().source, source);
                    // No construction capture produced precision metadata. The
                    // actual explicit cell must still keep the natural owner.
                    if let Some(patch) = build_patch(
                        &generator,
                        owner,
                        origin,
                        column.clone(),
                        &padded,
                        &fluids,
                        &FluidOccluders::new(),
                        &HashMap::new(),
                    ) {
                        areas[index] = top_area(&patch);
                        for triangle in patch.indices.chunks_exact(3) {
                            if patch
                                .normals
                                .get(triangle[0] as usize)
                                .is_none_or(|n| n[1] > 0.99)
                            {
                                for &i in triangle {
                                    assert_eq!(
                                        patch.positions[i as usize][1] + origin[1] as f32,
                                        level as f32
                                    );
                                }
                            }
                        }
                    }
                }
                assert_eq!(
                    areas,
                    [0., 256.],
                    "level={level} source={source} material={material}: explicit volume must neither duplicate nor remove the upper lake surface"
                );
            }
        }
    }
}

fn captured_patch(
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &TerraGenerator,
    position: SectionPos,
) -> Option<WaterPatch> {
    let generator = crate::SectionGenerator::new(terrain.clone());
    let column = Arc::new(
        generator
            .prepare_column([position.x, position.z], true)
            .unwrap(),
    );
    let origin = position.origin().unwrap();
    let mut padded = vec![0; 18 * 18 * 18];
    let mut fluids = journal.fluid_snapshot(position).unwrap();
    let mut levels = HashMap::new();
    for z in 0..18 {
        for y in 0..18 {
            for x in 0..18 {
                let p = [
                    origin[0] + x as i64 - 1,
                    origin[1] + y as i64 - 1,
                    origin[2] + z as i64 - 1,
                ];
                let material = journal
                    .block(p)
                    .or_else(|| chunks.block_at(p))
                    .unwrap_or_else(|| generator.block_at(p).unwrap());
                padded[x + 18 * (y + 18 * z)] = material;
                if crate::water::space::partial(material)
                    && let Some((p, level)) = capture_construction_water(
                        chunks,
                        journal,
                        terrain,
                        column.water_columns()[x + 18 * z].water_columns(),
                        p,
                        material,
                        y > 0,
                        &mut fluids,
                    )
                {
                    levels.insert(p, level);
                }
            }
        }
    }
    extend_construction_water(
        chunks,
        journal,
        terrain,
        origin,
        column.water_columns(),
        &mut fluids,
        &mut levels,
    );
    let occluders = capture_occluders(
        chunks,
        journal,
        &BlockPropertiesCatalog::default(),
        origin,
        &fluids,
    );
    build_patch(
        &generator, position, origin, column, &padded, &fluids, &occluders, &levels,
    )
}

fn profiled_lake_patch(material: u16, facing: u8, level: f64) -> WaterPatch {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: level - 4.,
        sea_level: level,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        biomes: false,
        caves: false,
        trees: false,
        rivers: false,
        landscape_revision: 0,
        structures_revision: 0,
        ..Default::default()
    })
    .unwrap();
    let p = [-8, level.ceil() as i64 - 1, -8];
    let owner = global_to_section(p).0;
    let generator = crate::SectionGenerator::new(terrain.clone());
    let mut chunks = ChunkStore::default();
    let mut data = generator.generate(owner).unwrap();
    data.set_block(global_to_section(p).1, material).unwrap();
    chunks.loaded.insert(
        owner,
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: vec![],
        },
    );
    let mut journal = EditJournal::default();
    journal.record(p, material).unwrap();
    journal
        .set_object_state(
            p,
            Some(BlockObjectState {
                facing,
                ..Default::default()
            }),
        )
        .unwrap();
    captured_patch(&chunks, &journal, &terrain, owner).unwrap()
}

fn corner_attributes(
    patch: &WaterPatch,
    owner: SectionPos,
    point: [f32; 3],
) -> Vec<(f32, [f32; 2], f32)> {
    let origin = owner.origin().unwrap().map(|p| p as f32);
    patch
        .indices
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter_map(|i| {
            let i = i as usize;
            (patch.normals.get(i).map_or(1., |n| n[1]) > 0.99
                && (0..3).all(|axis| {
                    (patch.positions[i][axis] + origin[axis] - point[axis]).abs() < 1e-5
                }))
            .then(|| {
                (
                    patch.wave_weights[i],
                    patch.flow[i],
                    patch.surface_effects.get(i).map_or(0., |e| e[3]),
                )
            })
        })
        .collect()
}

#[test]
fn independent_water_workers_keep_virtual_construction_fringe_waves_shared_across_both_axes_and_signs()
 {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 3.,
        sea_level: 8.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        biomes: false,
        caves: false,
        trees: false,
        rivers: false,
        landscape_revision: 0,
        structures_revision: 0,
        ..Default::default()
    })
    .unwrap();
    let generator = crate::SectionGenerator::new(terrain.clone());
    for axis in [0, 2] {
        for edge in [-16_i64, 16] {
            for offset in [1, -2] {
                let mut p = [8, 7, 8];
                p[axis] = edge + offset;
                let mut coordinate = [0, 0, 0];
                coordinate[axis] = edge.div_euclid(16) - 1;
                let left = SectionPos::new(coordinate[0], coordinate[1], coordinate[2]);
                coordinate[axis] += 1;
                let right = SectionPos::new(coordinate[0], coordinate[1], coordinate[2]);
                let mut chunks = ChunkStore::default();
                for owner in [left, right] {
                    let mut data = generator.generate(owner).unwrap();
                    if global_to_section(p).0 == owner {
                        data.set_block(global_to_section(p).1, plumbing::GLASS_PIPE)
                            .unwrap();
                    }
                    chunks.loaded.insert(
                        owner,
                        LoadedSection {
                            data,
                            mesh: None,
                            mesh_epoch: 0,
                            water_epoch: 0,
                            water: None,
                            props: vec![],
                        },
                    );
                }
                let mut journal = EditJournal::default();
                journal.record(p, plumbing::GLASS_PIPE).unwrap();
                journal
                    .set_object_state(
                        p,
                        Some(BlockObjectState {
                            facing: 4,
                            ..Default::default()
                        }),
                    )
                    .unwrap();
                // Each call constructs its OWN real 18^3 snapshot and natural
                // fringe. The far worker intentionally cannot see this pipe.
                let patches = [
                    captured_patch(&chunks, &journal, &terrain, left).unwrap(),
                    captured_patch(&chunks, &journal, &terrain, right).unwrap(),
                ];
                let mut point = [8., 8., 8.];
                point[axis] = edge as f32;
                for (owner, patch) in [left, right].into_iter().zip(&patches) {
                    let samples = corner_attributes(patch, owner, point);
                    assert!(!samples.is_empty());
                    assert!(
                        samples
                            .iter()
                            .all(|sample| sample.0 == 1. && sample.2 == 0.),
                        "virtual fringe must retain natural waves: axis={axis} edge={edge} offset={offset} owner={owner:?} samples={samples:?}"
                    );
                }
                let pipe_owner = global_to_section(p).0;
                let pipe_patch = &patches[usize::from(pipe_owner == right)];
                let contact = [p[0] as f32, 8., p[2] as f32];
                let contact = corner_attributes(pipe_patch, pipe_owner, contact);
                assert!(
                    !contact.is_empty() && contact.iter().all(|sample| sample.0 == 0.),
                    "real pipe contact must stay anchored"
                );
            }
        }
    }
}

#[test]
fn independently_meshed_current_and_still_water_share_the_shader_swell_inputs() {
    let terrain = TerraGenerator::new(WorldConfig {
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
    for axis in [0, 2] {
        for edge in [-16_i64, 16] {
            let mut coordinate = [0, 0, 0];
            coordinate[axis] = edge.div_euclid(16) - 1;
            let left = SectionPos::new(coordinate[0], 0, coordinate[2]);
            coordinate[axis] += 1;
            let right = SectionPos::new(coordinate[0], 0, coordinate[2]);
            let mut point = [8., 12., 8.];
            point[axis] = edge as f32;
            let mut shared = Vec::new();
            for owner in [left, right] {
                let origin = owner.origin().unwrap();
                let mut columns = vec![terrain.surface_column(0., 0.); 18 * 18];
                for z in 0..18 {
                    for x in 0..18 {
                        let global = [origin[0] + x as i64 - 1, 0, origin[2] + z as i64 - 1];
                        let column = &mut columns[x + 18 * z];
                        column.water_level = Some(12.);
                        column.water_flow = if global[axis] < edge {
                            [0.2, 0.]
                        } else {
                            [0.; 2]
                        };
                    }
                }
                let patch = crate::water::build_channel_patch(
                    owner,
                    origin,
                    &columns,
                    &vec![0; 18 * 18 * 18],
                )
                .unwrap();
                let samples = corner_attributes(&patch, owner, point);
                assert!(!samples.is_empty());
                shared.extend(samples);
            }
            let initial = shared[0];
            assert_eq!(initial.0, 1.);
            assert!(initial.1[0] > 0. && initial.1[0] < 0.2);
            assert!(
                shared.iter().all(|sample| *sample == initial),
                "per-cell current must not split identical weighted vertices: axis={axis} edge={edge}: {shared:?}"
            );
            // Above sea level COLOR.z is zero; all emitted horizontal effects
            // are normal(mask0). These are exactly the shader's swell inputs.
            for time in [0_f32, 0.37, 2.1, 9.7] {
                let displaced = |sample: (f32, [f32; 2], f32)| {
                    assert_eq!(sample.2, 0.);
                    let swell = if sample.1[0] * sample.1[0] + sample.1[1] * sample.1[1] > 0.01 {
                        0.4
                    } else {
                        0.18
                    };
                    (time * 2.1).sin() * 0.082 * sample.0 * swell
                };
                assert!(
                    shared
                        .iter()
                        .all(|sample| displaced(*sample) == displaced(initial))
                );
            }
        }
    }
}

#[test]
fn captured_vertical_valve_uses_its_octagonal_end_and_keeps_collision_corners_wet() {
    for facing in [4, 5, 12, 13] {
        let patch = profiled_lake_patch(plumbing::VALVE, facing, 8.);
        let expected = 256. - 4. * 0.155_f32.powi(2) * std::f32::consts::FRAC_PI_4.sin();
        assert!(
            (top_area(&patch) - expected).abs() < 0.002,
            "facing={facing}: square collision bounds must not cut the lake"
        );
        // This point lies inside the old .43 x .43 square and outside the
        // visible octagonal end, so a complete worker snapshot must keep it wet.
        let point = bevy::math::Vec2::new(8.641, 8.643);
        let count = patch
            .indices
            .chunks_exact(3)
            .filter(|triangle| {
                if patch.normals[triangle[0] as usize][1] < 0.99 {
                    return false;
                }
                let [a, b, c] = [triangle[0], triangle[1], triangle[2]].map(|i| {
                    let p = patch.positions[i as usize];
                    bevy::math::Vec2::new(p[0], p[2])
                });
                let sides = [
                    (b - a).perp_dot(point - a),
                    (c - b).perp_dot(point - b),
                    (a - c).perp_dot(point - c),
                ];
                sides.iter().all(|v| *v > 1e-7) || sides.iter().all(|v| *v < -1e-7)
            })
            .count();
        assert_eq!(
            count, 1,
            "the restored corner must have one surface, never zero or overlapping sheets"
        );
    }
}

#[test]
fn profiled_surface_contacts_and_split_edges_remain_sealed_during_waves_without_freezing_the_lake()
{
    use bevy::math::{Vec2, Vec3};
    for (material, facing) in [
        (plumbing::WOOD_PIPE, 0),
        (plumbing::STONE_PIPE, 4),
        (plumbing::IRON_PIPE, 1),
        (plumbing::GLASS_PIPE, 4),
        (plumbing::VALVE, 4),
        (plumbing::PUMP, 0),
    ] {
        let patch = profiled_lake_patch(material, facing, 7.5);
        let mut contacts = 0;
        for (index, point) in patch.positions.iter().enumerate() {
            if patch.normals[index][1] > 0.99
                && (8.0..=9.0).contains(&point[0])
                && (8.0..=9.0).contains(&point[2])
                && ((point[0] - point[0].round()).abs() > 0.001
                    || (point[2] - point[2].round()).abs() > 0.001)
            {
                contacts += 1;
                assert_eq!(
                    patch.wave_weights[index], 0.,
                    "moving contact vertex material={material}: {point:?}"
                );
            }
        }
        assert!(contacts > 0);
        assert!(
            patch
                .indices
                .iter()
                .any(|i| patch.wave_weights[*i as usize] > 0.9),
            "remote lake waves must remain enabled"
        );
        let local_triangles: Vec<_> = patch
            .indices
            .chunks_exact(3)
            .filter(|t| {
                patch.normals[t[0] as usize][1] > 0.99
                    && t.iter().all(|i| {
                        let p = patch.positions[*i as usize];
                        (6.0..=11.0).contains(&p[0]) && (6.0..=11.0).contains(&p[2])
                    })
            })
            .collect();
        let vertices: std::collections::BTreeSet<_> = local_triangles
            .iter()
            .flat_map(|t| t.iter().copied())
            .collect();
        for time in [0., 0.37, 2.1, 9.7] {
            // Bound the same vertical vertex displacement used by terra_water;
            // split edges must stay sealed throughout motion, not only at t=0.
            let displaced = |i: u32| {
                let p = Vec3::from_array(patch.positions[i as usize]);
                let wave = ((p.x * 0.74 - time * 4.1).sin() * 0.082
                    + (p.z * 1.38 + time * 2.9).sin() * 0.028)
                    * patch.wave_weights[i as usize];
                p.y + wave
            };
            for triangle in &local_triangles {
                for [a, b] in [[0, 1], [1, 2], [2, 0]] {
                    let [a, b] = [triangle[a], triangle[b]];
                    let pa = Vec3::from_array(patch.positions[a as usize]);
                    let pb = Vec3::from_array(patch.positions[b as usize]);
                    let edge = Vec2::new(pb.x - pa.x, pb.z - pa.z);
                    if edge.length_squared() < 1e-8 {
                        continue;
                    }
                    for &i in &vertices {
                        let p = Vec3::from_array(patch.positions[i as usize]);
                        let offset = Vec2::new(p.x - pa.x, p.z - pa.z);
                        let t = offset.dot(edge) / edge.length_squared();
                        if !(0.001..=0.999).contains(&t)
                            || (offset - edge * t).length_squared() > 1e-12
                        {
                            continue;
                        }
                        let expected = displaced(a) + (displaced(b) - displaced(a)) * t;
                        assert!(
                            (displaced(i) - expected).abs() < 1e-5,
                            "wave crack on split surface edge material={material} time={time} point={p:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn real_source_replacement_with_vertical_pipe_retains_flush_lake_surface_after_fluid_ticks() {
    use crate::streaming::{BlockEditRequest, commit_block_edit};
    for level in [7.3_f64, 8.0, 16.0, -16.0] {
        let terrain = TerraGenerator::new(WorldConfig {
            base_height: level - 4.,
            sea_level: level,
            hill_amplitude: 0.,
            mountain_amplitude: 0.,
            biomes: false,
            caves: false,
            trees: false,
            rivers: false,
            landscape_revision: 0,
            structures_revision: 0,
            ..Default::default()
        })
        .unwrap();
        let generator = crate::SectionGenerator::new(terrain.clone());
        let p = [-8, level.ceil() as i64 - 1, -8];
        let owner = global_to_section(p).0;
        let mut chunks = ChunkStore::default();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let position = owner.offset([dx, dy, dz]).unwrap();
                    chunks.loaded.insert(
                        position,
                        LoadedSection {
                            data: generator.generate(position).unwrap(),
                            mesh: None,
                            mesh_epoch: 0,
                            water_epoch: 0,
                            water: None,
                            props: vec![],
                        },
                    );
                }
            }
        }
        let mut journal = EditJournal::default();
        let properties = BlockPropertiesCatalog::default();
        for material in [27, plumbing::WOOD_PIPE] {
            let expected_material = chunks.block_at(p).unwrap();
            assert!(
                commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &BlockEditRequest {
                        position: p,
                        expected_material,
                        material
                    },
                    &properties
                )
                .unwrap()
            );
            if material == 27 {
                // This fixture represents the actual natural source level.
                // A deliberately placed full source would correctly stay full
                // when waterlogged, even above a fractional 7.3 lake surface.
                let natural = crate::water::fluid::from_columns(
                    terrain.water_columns(p[0] as f64 + 0.5, p[2] as f64 + 0.5),
                    p[1],
                );
                assert!(natural.source);
                journal.set_fluid(p, natural).unwrap();
            }
        }
        journal
            .set_object_state(
                p,
                Some(BlockObjectState {
                    facing: 4,
                    ..Default::default()
                }),
            )
            .unwrap();
        let above = [p[0], p[1] + 1, p[2]];
        let expected_material = chunks.block_at(above).unwrap();
        commit_block_edit(
            &mut chunks,
            &mut journal,
            &BlockEditRequest {
                position: above,
                expected_material,
                material: plumbing::WOOD_PIPE,
            },
            &properties,
        )
        .unwrap();
        journal
            .set_object_state(
                above,
                Some(BlockObjectState {
                    facing: 4,
                    ..Default::default()
                }),
            )
            .unwrap();
        for _ in 0..32 {
            crate::water::simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
        }
        let expected = crate::water::fluid::from_columns(
            terrain.water_columns(p[0] as f64 + 0.5, p[2] as f64 + 0.5),
            p[1],
        );
        assert_eq!(
            journal.fluid(p),
            Some(expected),
            "water outside pipe must retain the natural amount and source flag"
        );
        let mut area = 0.;
        for dy in 0..=1 {
            let position = owner.offset([0, dy, 0]).unwrap();
            let origin = position.origin().unwrap();
            let Some(patch) = captured_patch(&chunks, &journal, &terrain, position) else {
                continue;
            };
            area += top_area(&patch);
            for (&point, normal) in patch.positions.iter().zip(&patch.normals) {
                if normal[1] > 0.99 {
                    assert!(
                        (f64::from(point[1]) + origin[1] as f64 - level).abs() < 0.00001,
                        "level {level}: raised/depressed pipe water at {point:?}"
                    );
                }
            }
            for triangle in patch.indices.chunks_exact(3) {
                if patch.normals[triangle[0] as usize][1].abs() > 0.1 {
                    continue;
                }
                let points: [[f64; 3]; 3] = std::array::from_fn(|i| {
                    std::array::from_fn(|axis| {
                        f64::from(patch.positions[triangle[i] as usize][axis]) + origin[axis] as f64
                    })
                });
                let boundary = [0, 2].into_iter().any(|axis| {
                    [p[axis] as f64, p[axis] as f64 + 1.]
                        .into_iter()
                        .any(|plane| {
                            points
                                .iter()
                                .all(|point| (point[axis] - plane).abs() < 1e-5)
                        })
                });
                if boundary
                    && points.iter().all(|point| {
                        (point[0] - p[0] as f64 - 0.5).abs() < 1.01
                            && (point[2] - p[2] as f64 - 0.5).abs() < 1.01
                    })
                {
                    assert!(
                        points.iter().all(|point| point[1] < level - 0.1),
                        "box-shaped internal water curtain around pipe at {points:?}"
                    );
                }
            }
        }
        assert!(
            area > 255.85 && area <= 256.001,
            "only pipe cross-section may cover lake, level={level} area={area}"
        );
    }
}

#[test]
fn sparse_natural_water_around_a_submerged_pipe_has_no_internal_voxel_faces() {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 0.,
        sea_level: 10.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        biomes: false,
        caves: false,
        trees: false,
        rivers: false,
        landscape_revision: 0,
        structures_revision: 0,
        ..Default::default()
    })
    .unwrap();
    let generator = crate::SectionGenerator::new(terrain.clone());
    let owner = SectionPos::default();
    let column = Arc::new(generator.prepare_column([0, 0], true).unwrap());
    for material in [
        plumbing::WOOD_PIPE,
        plumbing::STONE_PIPE,
        plumbing::IRON_PIPE,
        plumbing::GLASS_PIPE,
    ] {
        for facing in 0..6 {
            let p = [8, 7, 8];
            let mut chunks = ChunkStore::default();
            chunks.loaded.insert(
                owner,
                LoadedSection {
                    data: BlockSection::empty(owner).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                },
            );
            chunks.set_prepared_column_for_test([0, 0], Some(column.clone()));
            let mut journal = EditJournal::default();
            let mut padded = vec![0; 18 * 18 * 18];
            for (q, id) in [
                (p, material),
                ([7, 7, 8], 3),
                ([8, 7, 7], 3),
                ([8, 6, 8], 3),
            ] {
                let (_, local) = global_to_section(q);
                chunks
                    .loaded
                    .get_mut(&owner)
                    .unwrap()
                    .data
                    .set_block(local, id)
                    .unwrap();
                journal.record(q, id).unwrap();
                padded
                    [(q[0] + 1) as usize + 18 * ((q[1] + 1) as usize + 18 * (q[2] + 1) as usize)] =
                    id;
            }
            journal
                .set_object_state(
                    p,
                    Some(BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                )
                .unwrap();
            journal
                .set_fluid(
                    p,
                    FluidCell {
                        amount: 8,
                        source: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            let mut fluids = journal.fluid_snapshot(owner).unwrap();
            let mut levels = HashMap::new();
            if let Some((q, level)) = capture_construction_water(
                &chunks,
                &journal,
                &terrain,
                column.surface_at(p[0], p[2]).unwrap().water_columns(),
                p,
                material,
                true,
                &mut fluids,
            ) {
                levels.insert(q, level);
            }
            extend_construction_water(
                &chunks,
                &journal,
                &terrain,
                [0; 3],
                column.water_columns(),
                &mut fluids,
                &mut levels,
            );
            let shapes = capture_occluders(&chunks, &journal, &Default::default(), [0; 3], &fluids);
            let patch = build_patch(
                &generator,
                owner,
                [0; 3],
                column.clone(),
                &padded,
                &fluids,
                &shapes,
                &levels,
            )
            .unwrap();
            for triangle in patch.indices.chunks_exact(3) {
                let normal = patch
                    .normals
                    .get(triangle[0] as usize)
                    .copied()
                    .unwrap_or([0., 1., 0.]);
                let center: [f32; 3] = std::array::from_fn(|axis| {
                    triangle
                        .iter()
                        .map(|i| patch.positions[*i as usize][axis])
                        .sum::<f32>()
                        / 3.
                });
                assert!(
                    normal[1].abs() > 0.01,
                    "fake submerged water/air boundary: material={material} facing={facing} center={center:?} normal={normal:?}"
                );
            }
        }
    }
}
