use super::*;
use crate::{SectionGenerator, TerraGenerator, edits::EditJournal};
use terra_core::WorldConfig;
use terra_voxel::{MaterialId, SectionPos};

#[test]
fn independent_surface_and_cave_reservoirs_leave_dry_rock_and_preserve_waterlogged_plants() {
    let columns = WaterColumns {
        intervals: [
            Some(WaterColumn {
                bed: 3.0,
                level: 8.0,
                depth: 5.0,
            }),
            Some(WaterColumn {
                bed: -40.0,
                level: -35.5,
                depth: 4.5,
            }),
            None,
        ],
    };
    for y in -45..=10 {
        let fluid = super::fluid::from_columns(columns, y);
        assert_eq!(
            fluid.amount > 0,
            (-40..=-36).contains(&y) || (3..=7).contains(&y)
        );
    }
    assert_eq!(super::fluid::from_columns(columns, -36).amount, 4);
    assert_eq!(columns.below(-34.0).unwrap().level, -35.5);
    assert_eq!(columns.below(10.0).unwrap().level, 8.0);
    let mut journal = EditJournal::default();
    let p = [-1, -16, -17];
    let source = FluidCell {
        amount: 8,
        source: true,
        flow: [0; 2],
    };
    journal.set_fluid(p, source).unwrap();
    journal.record(p, 50).unwrap();
    assert_eq!(journal.fluid(p), Some(source));
    journal.record(p, 0).unwrap();
    assert_eq!(journal.fluid(p), Some(source));
    journal.record(p, 3).unwrap();
    assert_eq!(journal.fluid(p).unwrap_or_default(), FluidCell::default());
    for material in 50..=55 {
        assert!(can_hold_fluid(material));
        journal.set_fluid(p, source).unwrap();
        journal.record(p, material).unwrap();
        assert_eq!(journal.fluid(p), Some(source));
    }
    assert!(!can_hold_fluid(49));
}

fn generator(height: f64, level: f64, water: bool) -> SectionGenerator {
    SectionGenerator::new(
        TerraGenerator::new(WorldConfig {
            base_height: height,
            sea_level: level,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            caves: false,
            trees: false,
            water,
            biomes: false,
            ..Default::default()
        })
        .unwrap(),
    )
}
fn patch(g: &SectionGenerator, p: SectionPos) -> WaterPatch {
    g.build(p, 0, &Default::default(), true)
        .unwrap()
        .water
        .unwrap()
}

#[test]
fn waterfall_lips_and_impacts_share_pinned_corners_with_moving_pool_interiors() {
    let g = generator(4.0, 8.0, true);
    let mut columns = vec![g.terrain().surface_column(0.0, 0.0); 18 * 18];
    for z in 0..18 {
        for x in 0..18 {
            columns[x + z * 18].water_level = Some(if x <= 8 { 20.0 } else { 8.0 });
        }
    }
    for y in [0, 1] {
        let position = SectionPos::new(-1, y, -1);
        let water = super::channels::build_channel_patch(
            position,
            position.origin().unwrap(),
            &columns,
            &vec![0; 18 * 18 * 18],
        )
        .unwrap();
        assert_eq!(water.wave_weights.len(), water.positions.len());
        let mut shared = std::collections::HashMap::new();
        let mut anchored = 0;
        let mut moving = 0;
        for (i, point) in water.positions.iter().enumerate() {
            let weight = water.wave_weights[i];
            assert!(weight.is_finite() && (0.0..=1.0).contains(&weight));
            if let Some(previous) = shared.insert(point.map(f32::to_bits), weight) {
                assert_eq!(
                    previous, weight,
                    "duplicate corner at {point:?} must deform identically"
                );
            }
            if (point[0] - 8.0).abs() < 0.001 {
                assert_eq!(weight, 0.0, "shared fall attachment stays anchored");
                anchored += 1;
            }
            if weight > 0.9 {
                moving += 1;
            }
            if water.surface_effects[i][3] == 1.0 {
                assert_eq!(weight, 0.0, "curtains remain pinned");
            }
        }
        assert!(anchored > 0 && moving > 0);
    }
}

#[test]
fn wave_weights_match_across_negative_chunk_boundaries_and_pin_live_fluid_edges() {
    let g = generator(4.0, 8.0, true);
    let make = |position: SectionPos| {
        let origin = position.origin().unwrap();
        let mut columns = vec![g.terrain().surface_column(0.0, 0.0); 18 * 18];
        for z in 0..18 {
            for x in 0..18 {
                let global_z = origin[2] + z as i64 - 1;
                columns[x + z * 18].water_level = Some(if global_z < -8 { 20.0 } else { 8.0 });
            }
        }
        let water = super::channels::build_channel_patch(
            position,
            origin,
            &columns,
            &vec![0; 18 * 18 * 18],
        )
        .unwrap();
        (water, columns)
    };
    let a_position = SectionPos::new(-1, 0, -1);
    let b_position = SectionPos::new(0, 0, -1);
    let (mut a, columns) = make(a_position);
    let (mut b, b_columns) = make(b_position);
    let edge = |water: &WaterPatch, x: f32| {
        water
            .positions
            .iter()
            .zip(&water.wave_weights)
            .filter(|(point, _)| (point[0] - x).abs() < 0.001)
            .map(|(point, &weight)| ((point[1].to_bits(), point[2].to_bits()), weight))
            .collect::<std::collections::HashMap<_, _>>()
    };
    assert_eq!(edge(&a, 16.0), edge(&b, 0.0));
    let fluids = [(
        [0, 7, -1],
        FluidCell {
            amount: 8,
            source: false,
            flow: [1, 0],
        },
    )]
    .into();
    super::geometry::assign_wave_weights(
        &mut a,
        a_position.origin().unwrap(),
        &columns,
        Some(&fluids),
    );
    super::geometry::assign_wave_weights(
        &mut b,
        b_position.origin().unwrap(),
        &b_columns,
        Some(&fluids),
    );
    assert_eq!(edge(&a, 16.0), edge(&b, 0.0));
    for (point, weight) in a.positions.iter().zip(&a.wave_weights) {
        if point == &[16.0, 8.0, 15.0] {
            assert_eq!(*weight, 0.0);
        }
    }
}

#[test]
fn water_is_a_horizontal_surface_not_a_voxel_material() {
    let g = generator(4.0, 8.0, true);
    let p = SectionPos::new(-1, 0, -1);
    let result = g.build(p, 0, &Default::default(), true).unwrap();
    assert_eq!(
        result.data.block([15, 8, 15]).unwrap(),
        MaterialId::Air as u16
    );
    let water = result.water.unwrap();
    assert_eq!(water.positions.len(), 17 * 17);
    assert_eq!(water.indices.len(), 16 * 16 * 6);
    assert!(water.positions.iter().all(|p| p[1] == 8.0));
    assert!(water.depth_shore.iter().all(|&uv| uv == [4.0, 0.0]));
    assert_eq!(water.wave_weights.len(), water.positions.len());
    assert!(water.wave_weights.iter().all(|&weight| weight == 1.0));
    for triangle in water.indices.chunks_exact(3) {
        let [a, b, c] =
            [triangle[0], triangle[1], triangle[2]].map(|i| water.positions[i as usize]);
        let normal_y = (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2]);
        assert!(normal_y > 0.0);
    }
    assert_eq!(
        g.terrain().water_column(-1.0, -1.0),
        Some(WaterColumn {
            level: 8.0,
            bed: 4.0,
            depth: 4.0
        })
    );
    assert_eq!(WATER_SURFACE_VERSION, 1);
}

#[test]
fn dry_disabled_and_headless_sections_do_not_allocate_water_meshes() {
    for g in [generator(12.0, 8.0, true), generator(4.0, 8.0, false)] {
        assert!(g.terrain().water_column(-1.0, -1.0).is_none());
        assert!(
            g.build(SectionPos::default(), 0, &Default::default(), true)
                .unwrap()
                .water
                .is_none()
        );
    }
    let g = generator(4.0, 8.0, true);
    assert!(
        g.build(SectionPos::default(), 0, &Default::default(), false)
            .unwrap()
            .water
            .is_none()
    );
    assert!(
        g.build(SectionPos::new(0, 1, 0), 0, &Default::default(), true)
            .unwrap()
            .water
            .is_none()
    );
}

#[test]
fn negative_fractional_levels_and_tile_seams_share_positions_and_depth() {
    let g = generator(-7.0, -0.25, true);
    let left = patch(&g, SectionPos::new(-1, -1, -1));
    let right = patch(&g, SectionPos::new(0, -1, -1));
    let far = patch(&g, SectionPos::new(-1, -1, 0));
    for i in 0..17 {
        assert_eq!(
            left.positions[16 + i * 17][1..],
            right.positions[i * 17][1..]
        );
        assert_eq!(left.depth_shore[16 + i * 17], right.depth_shore[i * 17]);
        assert_eq!(left.depth_shore[i + 16 * 17], far.depth_shore[i]);
        assert_eq!(left.positions[i + 16 * 17][0..2], far.positions[i][0..2]);
    }
    assert_eq!(left.positions[0][1] - 16.0, -0.25);
    assert_eq!(right, patch(&g, SectionPos::new(0, -1, -1)));
}

#[test]
fn waterfall_geometry_is_owned_once_and_clips_across_vertical_sections() {
    let terrain = generator(4.0, 8.0, true);
    let column = terrain.terrain().surface_column(0.5, 0.5);
    let mut columns = vec![column; 18 * 18];
    // A downstream tile with its upstream reach in the read-only padding.
    for column in columns.iter_mut().take(18) {
        column.water_level = Some(24.8);
        column.height = 21.0;
    }
    let padded = vec![0; 18 * 18 * 18];
    for sy in [0, 1] {
        let p = SectionPos::new(-1, sy, 0);
        let patch = super::channels::build_channel_patch(p, p.origin().unwrap(), &columns, &padded)
            .unwrap();
        assert_eq!(patch.positions.len(), patch.normals.len());
        assert_eq!(patch.positions.len(), patch.flow.len());
        assert!(
            patch
                .positions
                .iter()
                .all(|p| p.iter().all(|v| v.is_finite()) && p[1] >= 0.0 && p[1] <= 16.0)
        );
        assert!(
            patch
                .indices
                .iter()
                .all(|&i| (i as usize) < patch.positions.len())
        );
        assert_eq!(patch.positions.len(), patch.surface_effects.len());
        let mut offsets = Vec::new();
        for (point, effect) in patch.positions.iter().zip(&patch.surface_effects) {
            if effect[3] == 1.0 && effect[0] > 0.0 {
                assert!(
                    (0.05..0.96).contains(&point[2]),
                    "fall must clear the cliff and stay inside its downstream cell"
                );
                offsets.push(point[2]);
                assert!((effect[0] + effect[1] - effect[2]).abs() < 1e-5);
            }
        }
        let minimum = offsets.iter().copied().fold(f32::INFINITY, f32::min);
        let maximum = offsets.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            maximum - minimum > 0.45,
            "curtain must have a curved profile"
        );
        let middle = if sy == 0 { 15.0 } else { 1.0 };
        let lateral: Vec<_> = patch
            .positions
            .iter()
            .zip(&patch.surface_effects)
            .filter(|(p, e)| p[1] == middle && e[3] == 1.0)
            .map(|(p, _)| p[2])
            .collect();
        let near = lateral.iter().copied().fold(f32::INFINITY, f32::min);
        let far = lateral.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            far - near > 0.25,
            "falling streams need visible lateral relief"
        );
        for indices in patch.indices.chunks_exact(3) {
            let [a, b, c] =
                [indices[0], indices[1], indices[2]].map(|i| patch.positions[i as usize]);
            let ab: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
            let ac: [f32; 3] = std::array::from_fn(|i| c[i] - a[i]);
            let cross = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ];
            let normal = patch.normals[indices[0] as usize];
            assert!(cross.iter().zip(normal).map(|(a, b)| a * b).sum::<f32>() > 0.0);
        }
    }
}

#[test]
fn small_river_descents_share_corners_without_vertical_waterfall_strips() {
    let terrain = generator(4.0, 8.0, true);
    let base = terrain.terrain().surface_column(0.5, 0.5);
    let padded = vec![0; 18 * 18 * 18];
    let build = |position: SectionPos| {
        let origin = position.origin().unwrap();
        let columns: Vec<_> = (0..18 * 18)
            .map(|i| {
                let world_z = origin[2] as f64 + (i / 18) as f64 - 0.5;
                crate::SurfaceColumn {
                    water_level: Some(8.0 - world_z * 0.1),
                    ..base
                }
            })
            .collect();
        super::channels::build_channel_patch(position, origin, &columns, &padded).unwrap()
    };
    let left = build(SectionPos::new(-1, 0, -1));
    let right = build(SectionPos::new(0, 0, -1));
    let far = build(SectionPos::new(-1, 0, 0));
    for water in [&left, &right, &far] {
        assert_eq!(water.positions.len(), 16 * 16 * 4);
        assert!(water.normals.iter().all(|n| n[1] > 0.99));
        assert!(
            water
                .indices
                .iter()
                .all(|&i| (i as usize) < water.positions.len())
        );
    }
    for cell in 0..16 {
        let i = (cell * 16 + 15) * 4;
        let j = cell * 16 * 4;
        assert_eq!(left.positions[i + 3][1..], right.positions[j][1..]);
        assert_eq!(left.positions[i + 2][1..], right.positions[j + 1][1..]);
        let i = (15 * 16 + cell) * 4;
        let j = cell * 4;
        assert_eq!(left.positions[i + 1][..2], far.positions[j][..2]);
        assert_eq!(left.positions[i + 2][..2], far.positions[j + 3][..2]);
    }
}

#[test]
fn waterfall_sheets_start_at_exactly_five_blocks() {
    let terrain = generator(4.0, 8.0, true);
    let base = terrain.terrain().surface_column(0.5, 0.5);
    let padded = vec![0; 18 * 18 * 18];
    for drop in [1.0, 4.999, 5.0, 6.0, 8.0] {
        let mut columns = vec![base; 18 * 18];
        for column in columns.iter_mut().take(18) {
            column.water_level = Some(8.0 + drop);
        }
        let position = SectionPos::default();
        let water = super::channels::build_channel_patch(
            position,
            position.origin().unwrap(),
            &columns,
            &padded,
        )
        .unwrap();
        let falling = water.surface_effects.iter().any(|e| e[3] == 1.0);
        assert_eq!(falling, drop >= MIN_WATERFALL_DROP);
        if drop == 8.0 {
            // A crest exactly at section Y=16 must keep its connected lip in the
            // lower section; the next section has no positive-height fall face.
            let lip_vertices = water.positions.iter().filter(|p| p[1] == 16.0).count();
            assert_eq!(lip_vertices, 16 * (2 + 4));
            assert!(
                super::channels::build_channel_patch(
                    SectionPos::new(0, 1, 0),
                    [0, 16, 0],
                    &columns,
                    &padded
                )
                .is_none()
            );
        }
    }
}

#[test]
fn curved_waterfalls_share_world_space_edges_across_negative_sections() {
    let terrain = generator(-8.0, -4.0, true);
    let base = terrain.terrain().surface_column(0.5, 0.5);
    let padded = vec![0; 18 * 18 * 18];
    let build = |position: SectionPos| {
        let origin = position.origin().unwrap();
        let mut columns = vec![base; 18 * 18];
        for column in columns.iter_mut().take(18) {
            column.water_level = Some(20.0);
            column.height = 16.0;
        }
        super::channels::build_channel_patch(position, origin, &columns, &padded).unwrap()
    };
    let below = build(SectionPos::new(-1, -1, -1));
    let above = build(SectionPos::new(-1, 0, -1));
    let right = build(SectionPos::new(0, 0, -1));
    let edge_samples = |patch: &WaterPatch, axis: usize, value: f32, origin: [f32; 3]| {
        let mut samples: Vec<[u32; 7]> = patch
            .positions
            .iter()
            .zip(&patch.surface_effects)
            .filter(|(p, e)| p[axis] == value && e[3] == 1.0)
            .map(|(p, e)| {
                [
                    (p[0] + origin[0]).to_bits(),
                    (p[1] + origin[1]).to_bits(),
                    (p[2] + origin[2]).to_bits(),
                    e[0].to_bits(),
                    e[1].to_bits(),
                    e[2].to_bits(),
                    e[3].to_bits(),
                ]
            })
            .collect();
        samples.sort_unstable();
        samples.dedup();
        samples
    };
    let lower_edge = edge_samples(&below, 1, 16.0, [-16.0, -16.0, -16.0]);
    assert!(!lower_edge.is_empty());
    assert_eq!(
        lower_edge,
        edge_samples(&above, 1, 0.0, [-16.0, 0.0, -16.0])
    );
    let left_edge = edge_samples(&above, 0, 16.0, [-16.0, 0.0, -16.0]);
    assert!(!left_edge.is_empty());
    assert_eq!(left_edge, edge_samples(&right, 0, 0.0, [0.0, 0.0, -16.0]));
}

#[test]
fn thirty_meter_waterfall_is_continuous_across_three_vertical_sections() {
    let terrain = generator(-8.0, -4.0, true);
    let base = terrain.terrain().surface_column(0.5, 0.5);
    let mut columns = vec![base; 18 * 18];
    for column in columns.iter_mut().take(18) {
        column.water_level = Some(26.0);
        column.height = 22.0;
    }
    let padded = vec![0; 18 * 18 * 18];
    let mut previous_top: Option<Vec<[u32; 3]>> = None;
    for sy in -1..=1 {
        let position = SectionPos::new(-2, sy, -1);
        let water = super::channels::build_channel_patch(
            position,
            position.origin().unwrap(),
            &columns,
            &padded,
        )
        .unwrap();
        assert!(water.positions.iter().all(|p| (0.0..=16.0).contains(&p[1])));
        let edge = |height: f32| {
            let mut vertices: Vec<_> = water
                .positions
                .iter()
                .zip(&water.surface_effects)
                .filter(|(p, e)| p[1] == height && e[3] == 1.0)
                .map(|(p, e)| {
                    assert!((e[2] - 30.0).abs() < 1e-5);
                    [p[0].to_bits(), p[2].to_bits(), e[0].to_bits()]
                })
                .collect();
            vertices.sort_unstable();
            vertices.dedup();
            vertices
        };
        if let Some(previous) = previous_top {
            assert_eq!(previous, edge(0.0));
        }
        previous_top = Some(edge(16.0));
    }
}

#[test]
fn sloping_waterfall_lips_are_clipped_without_leaving_their_sections() {
    let terrain = generator(4.0, 8.0, true);
    let base = terrain.terrain().surface_column(0.5, 0.5);
    let mut columns = vec![base; 18 * 18];
    for (x, column) in columns.iter_mut().take(18).enumerate() {
        column.water_level = Some(15.2 + x as f64 * 0.12);
        column.height = 10.0;
    }
    let padded = vec![0; 18 * 18 * 18];
    for sy in [0, 1] {
        let position = SectionPos::new(-1, sy, -1);
        let water = super::channels::build_channel_patch(
            position,
            position.origin().unwrap(),
            &columns,
            &padded,
        )
        .unwrap();
        assert!(water.positions.iter().all(|p| (0.0..=16.0).contains(&p[1])));
        let lip = water
            .positions
            .iter()
            .zip(&water.surface_effects)
            .filter(|(_, e)| e[3] == 1.0 && e[0] == 0.0)
            .count();
        assert!(lip > 0, "both sections own their clipped part of the crest");
        assert!(
            water
                .indices
                .iter()
                .all(|&i| (i as usize) < water.positions.len())
        );
    }
}

#[test]
fn channel_shores_meet_dry_bed_and_share_depth_at_section_edges() {
    let terrain = generator(4.0, 8.0, true);
    let base = terrain.terrain().surface_column(0.5, 0.5);
    let padded = vec![0; 18 * 18 * 18];
    let build = |position: SectionPos| {
        let origin = position.origin().unwrap();
        let columns: Vec<_> = (0..18 * 18)
            .map(|i| {
                let world_z = origin[2] + (i / 18) as i64 - 1;
                crate::SurfaceColumn {
                    height: if world_z < 0 { 7.0 } else { 4.0 },
                    water_level: (world_z >= 0).then_some(8.0),
                    ..base
                }
            })
            .collect();
        super::channels::build_channel_patch(position, origin, &columns, &padded).unwrap()
    };
    let left = build(SectionPos::new(-1, 0, 0));
    let right = build(SectionPos::new(0, 0, 0));
    assert!(
        left.positions
            .iter()
            .filter(|p| p[2] == 0.0)
            .all(|p| p[1] == 7.0)
    );
    assert_eq!(left.positions[15 * 4 + 3][1..], right.positions[0][1..]);
    assert_eq!(left.depth_shore[15 * 4 + 3], right.depth_shore[0]);
    assert_eq!(left.depth_shore[15 * 4 + 2], right.depth_shore[1]);
    assert!(left.surface_effects.iter().all(|e| *e == [0.0; 4]));
}

#[test]
fn intersecting_gentle_slopes_use_one_corner_for_every_incident_cell() {
    let terrain = generator(0.0, 1.0, true);
    let base = terrain.terrain().surface_column(0.5, 0.5);
    let mut columns = vec![base; 18 * 18];
    for (x, z, level) in [(1, 1, 1.0), (2, 1, 4.0), (1, 2, 4.0), (2, 2, 7.0)] {
        columns[x + 18 * z].water_level = Some(level);
    }
    let water = super::channels::build_channel_patch(
        SectionPos::default(),
        [0; 3],
        &columns,
        &vec![0; 18 * 18 * 18],
    )
    .unwrap();
    let heights: Vec<_> = water
        .positions
        .iter()
        .filter(|p| p[0] == 1.0 && p[2] == 1.0)
        .map(|p| p[1])
        .collect();
    assert_eq!(heights, vec![4.0; 4]);
}

#[test]
fn bank_edits_rebuild_shared_water_corners_and_survive_reload() {
    let g = generator(4.0, 8.0, true);
    let p = SectionPos::new(-1, 0, -1);
    let base = patch(&g, p);
    let mut edits = EditJournal::default();
    let position = [-1, 8, -1];
    let affected = edits.record(position, MaterialId::Stone as u16).unwrap();
    assert!(affected.contains(&SectionPos::new(0, 0, 0)));
    let changed = g
        .build(p, edits.revision(p), &edits.snapshot(p).unwrap(), true)
        .unwrap()
        .water
        .unwrap();
    assert_eq!(changed.indices.len(), base.indices.len() - 6);
    assert_ne!(
        changed.depth_shore[16 + 16 * 17],
        base.depth_shore[16 + 16 * 17]
    );
    let diagonal = g
        .build(
            SectionPos::default(),
            0,
            &edits.snapshot(SectionPos::default()).unwrap(),
            true,
        )
        .unwrap()
        .water
        .unwrap();
    assert_eq!(changed.depth_shore[16 + 16 * 17], diagonal.depth_shore[0]);
    assert_eq!(
        changed,
        g.build(p, edits.revision(p), &edits.snapshot(p).unwrap(), true)
            .unwrap()
            .water
            .unwrap()
    );
    edits.record(position, 0).unwrap();
    assert_eq!(
        base,
        g.build(p, edits.revision(p), &edits.snapshot(p).unwrap(), true)
            .unwrap()
            .water
            .unwrap()
    );
}
