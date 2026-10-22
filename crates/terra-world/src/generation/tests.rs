use super::*;
use terra_core::WorldConfig;
use terra_core::WorldSeed;
use terra_voxel::{ChunkPos, MaterialId, SAMPLE_COUNT, SAMPLE_SIZE};
fn generator(seed: u64) -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        seed: WorldSeed(seed),
        ..WorldConfig::default()
    })
    .unwrap()
}
#[test]
fn seed_determinism_and_variation() {
    let a = generator(42);
    let b = generator(42);
    let c = generator(43);
    let d = generator(42 + (1 << 32));
    let positions = [[0.0, 16.0, 0.0], [-1.0, -10.0, -1.0], [123.0, 45.0, -456.0]];
    for p in positions {
        assert_eq!(a.sample(p), b.sample(p));
    }
    assert!(positions.iter().any(|&p| a.sample(p) != c.sample(p)));
    assert!(positions.iter().any(|&p| a.sample(p) != d.sample(p)));
    assert_eq!(a.version(), terra_core::GENERATOR_VERSION);
}
#[test]
fn all_axes_have_identical_border_samples() {
    let generator = ChunkGenerator::new(generator(123));
    let p = ChunkPos::new(-1, -1, -1);
    let center = generator.generate(p).unwrap();
    for axis in 0..3 {
        let neighbor = match axis {
            0 => ChunkPos::new(0, -1, -1),
            1 => ChunkPos::new(-1, 0, -1),
            _ => ChunkPos::new(-1, -1, 0),
        };
        let next = generator.generate(neighbor).unwrap();
        for a in 0..SAMPLE_SIZE {
            for b in 0..SAMPLE_SIZE {
                let mut left = [0; 3];
                left[axis] = 32;
                left[(axis + 1) % 3] = a;
                left[(axis + 2) % 3] = b;
                let mut right = left;
                right[axis] = 0;
                assert_eq!(center.sample(left).unwrap(), next.sample(right).unwrap());
            }
        }
    }
}
#[test]
fn negative_chunks_match_global_minus_one() {
    let generator = ChunkGenerator::new(generator(42));
    let chunk = generator.generate(ChunkPos::new(-1, -1, -1)).unwrap();
    assert_eq!(chunk.samples().len(), SAMPLE_COUNT);
    assert_eq!(
        chunk.sample([31; 3]).unwrap(),
        generator.terrain().sample([-1.0; 3])
    );
    assert!(chunk.samples().iter().all(|s| s.density.is_finite()));
}
#[test]
fn generation_order_has_no_effect() {
    let generator = ChunkGenerator::new(generator(99));
    let a = ChunkPos::new(-3, 0, 2);
    let b = ChunkPos::new(2, -1, -4);
    let first_a = generator.generate(a).unwrap();
    let first_b = generator.generate(b).unwrap();
    assert_eq!(first_b, generator.generate(b).unwrap());
    assert_eq!(first_a, generator.generate(a).unwrap());
}
#[test]
fn materials_and_density_convention() {
    let g = TerraGenerator::new(WorldConfig {
        caves: false,
        biomes: false,
        trees: false,
        water: false,
        base_height: 24.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        ..WorldConfig::default()
    })
    .unwrap();
    let h = g.surface_height(-1.0, -1.0);
    assert_eq!(
        g.sample([-1.0, h + 1.0, -1.0]).material,
        MaterialId::Air as u16
    );
    assert_eq!(
        g.sample([-1.0, h - 10.0, -1.0]).material,
        MaterialId::Stone as u16
    );
    assert_eq!(
        g.sample([-1.0, h - 3.0, -1.0]).material,
        MaterialId::Dirt as u16
    );
    assert!(g.sample([-1.0, h - 1.0, -1.0]).density < 0.0);
    assert_eq!(g.sample([-1.0, h, -1.0]).density, 0.0);
    assert_eq!(g.material_at(20.0, 0.5, -0.5), MaterialId::Grass);
    assert_eq!(g.material_at(7.0, 0.5, -0.5), MaterialId::Sand);
    assert_eq!(
        g.material_at(g.config().snow_height + 1.0, 0.5, -0.5),
        MaterialId::Snow
    );
}
#[test]
fn caves_carve_solid_terrain_without_opening_the_roof() {
    let caves = TerraGenerator::new(WorldConfig {
        seed: WorldSeed(42),
        cave_structure_revision: 1,
        ..Default::default()
    })
    .unwrap();
    let solid = TerraGenerator::new(WorldConfig {
        seed: WorldSeed(42),
        caves: false,
        ..WorldConfig::default()
    })
    .unwrap();
    let mut carved = 0;
    for z in -12..12 {
        for y in -16..0 {
            for x in -12..12 {
                let p = [x as f64 * 3.0, y as f64 * 3.0, z as f64 * 3.0];
                if solid.sample(p).density < 0.0 && caves.sample(p).density > 0.0 {
                    carved += 1;
                    assert_eq!(caves.sample(p).material, MaterialId::Air as u16);
                }
            }
        }
    }
    assert!(carved > 0);
    let h = caves.surface_height(0.0, 0.0);
    assert_eq!(
        caves.sample([0.0, h - 2.0, 0.0]),
        solid.sample([0.0, h - 2.0, 0.0])
    );
}
#[test]
fn generation_checks_extreme_positions() {
    let generator = ChunkGenerator::new(generator(1));
    assert!(matches!(
        generator.generate(ChunkPos::new(i64::MAX, 0, 0)),
        Err(GenerationError::Voxel(_))
    ));
    assert!(matches!(
        generator.generate(ChunkPos::new(MAX_SAMPLE_COORDINATE / 32, 0, 0)),
        Err(GenerationError::PrecisionLimit(_))
    ));
}

#[test]
fn cave_density_is_continuous_at_the_roof_and_culling_threshold() {
    let g = TerraGenerator::new(WorldConfig {
        seed: WorldSeed(42),
        cave_structure_revision: 1,
        ..Default::default()
    })
    .unwrap();
    let mut intersections = 0;
    for z in -12..12 {
        for x in -12..12 {
            let x = f64::from(x) * 3.0;
            let z = f64::from(z) * 3.0;
            let height = g.surface_height(x, z);
            for depth in [2.5, 5.0] {
                let above = g.sample([x, height - depth + 0.001, z]).density;
                let below = g.sample([x, height - depth - 0.001, z]).density;
                assert!((above - below).abs() < 0.01, "{x}, {z}: {above} -> {below}");
            }
            if g.sample([x, height - 5.0, z]).density.abs() < 0.01 {
                intersections += 1;
            }
        }
    }
    assert!(
        intersections > 0,
        "test region must exercise actual cave-roof intersections"
    );
}
