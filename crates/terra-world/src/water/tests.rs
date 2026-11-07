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
fn natural_reservoir_meshes_never_project_waterfalls_through_dry_air() {
    let g = generator(4.0, 8.0, true);
    for drop in [1.0, 4.0, 5.0, 12.0, 30.0] {
        let mut columns = vec![g.terrain().surface_column(0.0, 0.0); 18 * 18];
        for z in 0..18 {
            for x in 0..18 {
                columns[x + z * 18].water_level = Some(if x <= 8 { 8.0 + drop } else { 8.0 });
            }
        }
        let mut surfaces = 0;
        for sy in 0..=2 {
            let owner = SectionPos::new(-1, sy, -1);
            if let Some(patch) = super::channels::build_channel_patch(
                owner,
                owner.origin().unwrap(),
                &columns,
                &vec![0; 18 * 18 * 18],
            ) {
                surfaces += 1;
                assert!(patch.surface_effects.iter().all(|e| e[3] != 1.0));
                assert!(patch.normals.iter().all(|normal| normal[1] > 0.01));
                assert!(patch.positions.iter().all(|p| (0.0..=16.0).contains(&p[1])));
                assert!(
                    patch
                        .indices
                        .iter()
                        .all(|&i| (i as usize) < patch.positions.len())
                );
            }
        }
        assert!(
            surfaces > 0,
            "reservoir tops remain visible while falling geometry belongs to cells"
        );
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
        &vec![0; 18 * 18 * 18],
    );
    super::geometry::assign_wave_weights(
        &mut b,
        b_position.origin().unwrap(),
        &b_columns,
        Some(&fluids),
        &vec![0; 18 * 18 * 18],
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
    for (x, z, level) in [(1, 1, 1.0), (2, 1, 1.375), (1, 2, 1.375), (2, 2, 1.75)] {
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
    assert_eq!(heights, vec![1.375; 4]);
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

#[test]
fn shoreline_waves_are_pinned_and_live_solid_tops_do_not_get_a_coplanar_water_sheet() {
    let g = generator(4.0, 8.0, false);
    for section in [SectionPos::default(), SectionPos::new(-1, 0, -1)] {
        let origin = section.origin().unwrap();
        let mut columns = vec![g.terrain().surface_column(0.5, 0.5); 18 * 18];
        for c in &mut columns {
            c.height = 4.0;
            c.water_level = Some(8.0);
        }
        let mut padded = vec![0; 18 * 18 * 18];
        // A player-placed grass block reaches the water plane. It must not
        // acquire an overlapping transparent lake quad or a moving contact edge.
        padded[9 + 18 * (8 + 18 * 9)] = 1;
        let patch = super::build_water_patch(section, origin, 8.0, &columns, &padded).unwrap();
        assert_eq!(patch.indices.len() / 3, 510);
        for (p, weight) in patch.positions.iter().zip(&patch.wave_weights) {
            if (p[0] == 8.0 || p[0] == 9.0) && (p[2] == 8.0 || p[2] == 9.0) {
                assert_eq!(*weight, 0.0);
            }
        }
        assert!(
            patch.wave_weights.iter().any(|w| *w > 0.9),
            "open water retains its waves"
        );
        columns[9 + 18 * 9].water_level = None;
        assert_eq!(super::channels::shared_wave_weight(&columns, 8, 8), 0.0);
    }
}
