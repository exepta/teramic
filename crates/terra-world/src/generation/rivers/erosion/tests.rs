use super::*;
use crate::{SectionGenerator, TerraGenerator};
use terra_core::{WorldConfig, WorldSeed};

fn terrain(seed: u64, revision: u8) -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        seed: WorldSeed(seed),
        river_terrain_revision: revision,
        rivers: true,
        caves: false,
        trees: false,
        structures_revision: 0,
        ..Default::default()
    })
    .unwrap()
}

fn center(path: &RiverPath, t: f64) -> [f64; 2] {
    let index = (t / STEP).floor() as usize;
    let next = (index + 1).min(POINTS - 1);
    let along = (t / STEP).fract();
    [
        path.points[index][0] + (path.points[next][0] - path.points[index][0]) * along,
        path.points[0][2] + t,
    ]
}

fn fixture(legacy: &TerraGenerator, modern: &TerraGenerator) -> (RiverPath, usize) {
    for radius in 0_i64..=8 {
        for z in -radius..=radius {
            for x in -radius..=radius {
                if radius > 0 && x.abs() != radius && z.abs() != radius {
                    continue;
                }
                let cell = [x, z];
                if !legacy.river_enabled(cell) {
                    continue;
                }
                let path = legacy.build_river(cell);
                for (index, pair) in path.points.windows(2).enumerate() {
                    if pair[0][1] - pair[1][1] < 12.0 {
                        continue;
                    }
                    let [x, z] = center(&path, (index as f64 + 0.5) * STEP + 2.0);
                    let before = legacy.river_at(x, z).unwrap();
                    let after = modern.river_at(x, z).unwrap();
                    if before.roof_blend > 0.6
                        && after.roof_blend < before.roof_blend * 0.5
                        && (x < 0.0 || z < 0.0)
                    {
                        return (path, index);
                    }
                }
            }
        }
    }
    panic!(
        "seed {} has no actual covered mountain cataract in the bounded fixture",
        legacy.config().seed.0
    );
}

#[test]
fn plunge_openings_are_wide_asymmetric_local_and_preserve_the_hydraulic_lip() {
    let path = RiverPath {
        points: std::array::from_fn(|i| [0.0, if i == 0 { 46.0 } else { 10.0 }, i as f64 * STEP]),
        ground: [58.0; POINTS],
        extensions: [0.0; 2],
        width: 5.0,
        phase: 0.3,
        branch: None,
        brook: false,
    };
    let edge = STEP * 0.5;
    assert_eq!(path.fall_erosion(0, edge + 2.0, 0.0, 5.0), 1.0);
    for lateral in [-12.0, 12.0] {
        assert!(path.fall_erosion(0, edge + 2.0, lateral, 5.0) > 0.6);
    }
    for lateral in [-64.0, 64.0] {
        assert_eq!(path.fall_erosion(0, edge + 2.0, lateral, 5.0), 0.0);
    }
    assert_eq!(path.fall_erosion(0, edge - 14.0, 0.0, 5.0), 0.0);
    assert_eq!(path.fall_erosion(7, edge + 56.0, 0.0, 5.0), 0.0);
    assert_ne!(
        path.fall_erosion(0, edge + 2.0, -20.0, 5.0),
        path.fall_erosion(0, edge + 2.0, 20.0, 5.0)
    );
    assert!(
        (path.fall_erosion(0, edge - 1e-6, 15.0, 5.0)
            - path.fall_erosion(0, edge + 1e-6, 15.0, 5.0))
        .abs()
            < 1e-5
    );
    assert_eq!(path.level(0, 0.499), 46.0);
    assert_eq!(path.level(0, 0.5), 10.0);
    let buried = RiverPath {
        ground: [160.0; POINTS],
        ..path
    };
    assert_eq!(buried.fall_erosion(0, edge + 2.0, 0.0, 5.0), 0.0);
    assert_eq!(buried.level(0, 0.499), 46.0);
    assert_eq!(buried.level(0, 0.5), 10.0);
}

#[test]
fn actual_cataracts_remove_solid_trench_banks_and_expose_rock_across_three_seeds() {
    for seed in [42042, 1337, 99173] {
        let legacy = terrain(seed, 0);
        let modern = terrain(seed, 1);
        let (path, index) = fixture(&legacy, &modern);
        let edge = (index as f64 + 0.5) * STEP;
        let mut removed = 0;
        let mut stripped = 0;
        let mut widened = 0;
        for dt in [2.0, 8.0, 16.0, 24.0] {
            let [cx, z] = center(&path, (edge + dt).min((POINTS - 1) as f64 * STEP));
            assert_eq!(legacy.water_column(cx, z), modern.water_column(cx, z));
            for offset in [-24.0, -20.0, -16.0, -12.0, 12.0, 16.0, 20.0, 24.0] {
                let x = (cx + offset).floor() + 0.5;
                let z = z.floor() + 0.5;
                let before_river = legacy.river_at(x, z).unwrap();
                let after_river = modern.river_at(x, z).unwrap();
                assert_eq!(before_river.fall_erosion, 0.0);
                assert_eq!(before_river.level, after_river.level);
                assert_eq!(before_river.width, after_river.width);
                assert_eq!(before_river.depth, after_river.depth);
                assert_eq!(before_river.flow, after_river.flow);
                let before = legacy.surface_column(x, z);
                let after = modern.surface_column(x, z);
                let raw = modern.base_height_with_biome(x, z, modern.climate_biome_at(x, z));
                assert!(after.height <= raw + 1e-9, "erosion must never raise a dam");
                if before_river.distance <= before_river.width + 2.0 {
                    continue;
                }
                if before.height - after.height > 1.0 {
                    widened += 1;
                }
                if after.soil_depth < 2.0 && after.soil_depth < before.soil_depth {
                    let y = (after.height - 0.5).floor() - 2.0 + 0.5;
                    let sample = modern.sample_with_column([x, y, z], after);
                    assert!(sample.density <= 0.0);
                    assert!(
                        matches!(sample.material, 3 | 11 | 13),
                        "eroded mountain bank must expose real rock, got {sample:?}"
                    );
                    stripped += 1;
                }
                for y in after.height.ceil() as i64..=before.height.floor() as i64 {
                    let p = [x, y as f64 + 0.5, z];
                    removed += usize::from(
                        legacy.sample_with_column(p, before).density <= 0.0
                            && modern.sample_with_column(p, after).density > 0.0,
                    );
                }
            }
        }
        let [x, z] = center(&path, edge + 2.0);
        eprintln!(
            "waterfall terrain seed={seed} x={x:.1} z={z:.1} removed_solid_voxels={removed} widened_columns={widened} rock_columns={stripped}"
        );
        assert!(removed > 24, "seed {seed}: actual bank volume must change");
        assert!(
            widened >= 4,
            "seed {seed}: amphitheatre must exceed the wet channel"
        );
        assert!(
            stripped > 0,
            "seed {seed}: steep eroded banks need exposed rock"
        );
    }
}

#[test]
fn unaffected_reaches_and_legacy_revision_keep_exact_height_and_material_samples() {
    let legacy = terrain(42042, 0);
    let modern = terrain(42042, 1);
    let mut checked = 0;
    for cell in [[0, 0], [-1, -1], [1, -2]] {
        let path = legacy.build_river(cell);
        for t in (0..(POINTS - 1) * STEP as usize).step_by(11) {
            let [cx, z] = center(&path, t as f64);
            for lateral in [-80.0, -48.0, -24.0, 0.0, 24.0, 48.0, 80.0] {
                let x = cx + lateral;
                if [[0.0, 0.0], [-1.0, 0.0], [1.0, 0.0], [0.0, -1.0], [0.0, 1.0]]
                    .into_iter()
                    .any(|[dx, dz]| {
                        modern
                            .river_at(x + dx, z + dz)
                            .is_some_and(|r| r.fall_erosion > 0.0)
                    })
                {
                    continue;
                }
                let a = legacy.surface_column(x, z);
                let b = modern.surface_column(x, z);
                assert_eq!(a.height.to_bits(), b.height.to_bits());
                assert_eq!(a.soil_depth.to_bits(), b.soil_depth.to_bits());
                assert_eq!(a.surface_material, b.surface_material);
                for depth in [0.25, 2.0, 8.0] {
                    let p = [x, a.height - depth, z];
                    assert_eq!(
                        legacy.sample_with_column(p, a),
                        modern.sample_with_column(p, b)
                    );
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 100);
}

#[test]
fn waterfall_section_data_is_deterministic_across_negative_seams_and_generation_order() {
    let legacy = terrain(42042, 0);
    let terrain = terrain(42042, 1);
    let (path, index) = fixture(&legacy, &terrain);
    let [x, z] = center(&path, (index as f64 + 0.5) * STEP + 8.0);
    assert!(x < 0.0 || z < 0.0);
    let p = [
        (x.floor() as i64).div_euclid(16) * 16,
        path.points[index + 1][1].floor() as i64,
        z.floor() as i64,
    ];
    let owner = terra_voxel::global_to_section(p).0;
    let left = terra_voxel::SectionPos::new(owner.x - 1, owner.y, owner.z);
    let forward = SectionGenerator::new(terrain.clone());
    let reverse = SectionGenerator::new(TerraGenerator::new(terrain.config().clone()).unwrap());
    let a = forward.build(left, 0, &Default::default(), true).unwrap();
    let b = forward.build(owner, 0, &Default::default(), false).unwrap();
    let rb = reverse.build(owner, 0, &Default::default(), true).unwrap();
    let ra = reverse.build(left, 0, &Default::default(), false).unwrap();
    assert_eq!(a.data, ra.data);
    assert_eq!(b.data, rb.data);
    for y in 0..16 {
        for z in 0..16 {
            for (section, local) in [(left, [15, y, z]), (owner, [0, y, z])] {
                let global = section.global_block(local).unwrap();
                let data = if section == left { &a.data } else { &b.data };
                assert_eq!(
                    data.block(local).unwrap(),
                    forward.block_at(global).unwrap()
                );
            }
        }
    }
}
