use super::biomes::BiomeKind;
use super::*;
use terra_core::WorldConfig;
use terra_voxel::{MaterialId, global_to_section};

#[test]
fn new_cave_dirt_forms_connected_patches_not_checkerboard_noise() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 4,
        sea_level: 22.0,
        rivers: true,
        biome_definitions: Some(terra_core::BiomeDefinitions::default()),
        ..Default::default()
    })
    .unwrap();
    let mut dirt = std::collections::HashSet::new();
    for z in -24..24 {
        for x in -24..24 {
            let column = terrain.surface_column(f64::from(x) + 0.5, f64::from(z) + 0.5);
            for y in -48..0 {
                let sample = terrain.sample_with_column(
                    [f64::from(x) + 0.5, f64::from(y) + 0.5, f64::from(z) + 0.5],
                    column,
                );
                if sample.density <= 0.0 && sample.material == MaterialId::Dirt as u16 {
                    dirt.insert([x, y, z]);
                }
            }
        }
    }
    assert!(dirt.len() > 30, "deep dirt fixture: {}", dirt.len());
    let connected = dirt
        .iter()
        .filter(|p| {
            [
                [1, 0, 0],
                [-1, 0, 0],
                [0, 1, 0],
                [0, -1, 0],
                [0, 0, 1],
                [0, 0, -1],
            ]
            .iter()
            .any(|d| dirt.contains(&std::array::from_fn(|i| p[i] + d[i])))
        })
        .count();
    assert!(
        connected * 10 > dirt.len() * 9,
        "dirt must form connected patches"
    );
}

#[test]
fn cliffted_desert_is_a_normalized_desert_interior_with_rare_salt_chambers() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 3,
        sea_level: 8.0,
        cave_structure_revision: 2,
        ..Default::default()
    })
    .unwrap();
    let mut eligible = 0;
    let mut salt = 0;
    let mut example = None;
    for z in -96..=96 {
        for x in -96..=96 {
            let px = f64::from(x) * 192.0 + 96.0;
            let pz = f64::from(z) * 192.0 + 96.0;
            let biome = terrain.biome_at(px, pz);
            assert!((biome.weights.values().iter().sum::<f64>() - 1.0).abs() < 1e-10);
            let parent = terrain.climate.sample(px, pz);
            if biome.weights.cliffted_desert > 0.0 {
                assert!(parent.weights.desert > 0.7);
                assert!((biome.aridity() - parent.weights.desert).abs() < 1e-10);
            }
            if biome.weights.dominant() == BiomeKind::ClifftedDesert {
                eligible += 1;
            }
            if let Some(center) = terrain.salt_chamber_center(px, pz) {
                assert_eq!(biome.weights.dominant(), BiomeKind::ClifftedDesert);
                assert_eq!(terrain.cave_kind_at(px, pz), CaveKind::Dripstone);
                salt += 1;
                example.get_or_insert(center);
            }
        }
    }
    let frequency = salt as f64 / f64::from(eligible.max(1));
    println!("eligible={eligible} salt={salt} fraction={frequency} example={example:?}");
    assert!(eligible > 40 && salt > 0);
    assert!((0.05..0.15).contains(&frequency));
    // A cliff patch is nested inside the old desert envelope. Its outside is
    // reached through ordinary desert, never through a new detached dry island.
    let [x, _, z] = example.unwrap();
    assert!((1..=24).any(|step| {
        [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)]
            .iter()
            .any(|&(dx, dz)| {
                terrain
                    .biome_at(
                        x + dx * f64::from(step) * 64.0,
                        z + dz * f64::from(step) * 64.0,
                    )
                    .weights
                    .dominant()
                    == BiomeKind::Desert
            })
    }));
}

#[test]
fn salt_cave_is_broad_and_only_its_native_dripstone_walls_contain_salt() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 4,
        sea_level: 22.0,
        rivers: true,
        biome_definitions: Some(terra_core::BiomeDefinitions::default()),
        cave_structure_revision: 2,
        trees: false,
        ..Default::default()
    })
    .unwrap();
    let center = (-96..=96)
        .find_map(|z| {
            (-96..=96).find_map(|x| {
                terrain
                    .salt_chamber_center(f64::from(x) * 192.0 + 96.0, f64::from(z) * 192.0 + 96.0)
            })
        })
        .unwrap();
    assert!(terrain.sample(center).density > 0.0);
    let width = (-80..=80)
        .filter(|dx| {
            terrain
                .sample([center[0] + f64::from(*dx), center[1], center[2]])
                .density
                > 0.0
        })
        .count();
    assert!(width >= 80, "salt width={width}");
    let generator = SectionGenerator::new(terrain.clone());
    let owner = global_to_section(center.map(|v| v.floor() as i64)).0;
    let mut salt = 0;
    for dz in -2..=2 {
        for dx in -2..=2 {
            for dy in -2..=2 {
                let p = owner.offset([dx, dy, dz]).unwrap();
                let section = generator.generate(p).unwrap();
                for (index, &material) in section.blocks().iter().enumerate() {
                    if material == MaterialId::SaltOre as u16 {
                        let global = p
                            .global_block([index % 16, index / 16 % 16, index / 256])
                            .unwrap();
                        assert_eq!(
                            terrain
                                .biome_at(global[0] as f64 + 0.5, global[2] as f64 + 0.5)
                                .weights
                                .dominant(),
                            BiomeKind::ClifftedDesert
                        );
                        assert_eq!(generator.block_at(global).unwrap(), material);
                        salt += 1;
                    }
                }
            }
        }
    }
    println!("natural_salt_center={center:?} width={width} salt_blocks={salt}");
    assert!(salt > 50);
    for p in [[0, -48, 0], [-192, -32, -192], [512, -48, 512]] {
        let section = generator.generate(global_to_section(p).0).unwrap();
        assert!(!section.blocks().contains(&(MaterialId::SaltOre as u16)));
    }
}

#[test]
fn all_three_native_cave_types_exist_without_chamber_seams_or_open_surface_roofs() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 4,
        sea_level: 22.0,
        rivers: true,
        biome_definitions: Some(terra_core::BiomeDefinitions::default()),
        ..Default::default()
    })
    .unwrap();
    let mut found = [false; 3];
    for z in -5..=5 {
        for x in -5..=5 {
            let px = f64::from(x) * 192.0 + 96.0;
            let pz = f64::from(z) * 192.0 + 96.0;
            let kind = terrain.cave_kind_at(px, pz);
            found[match kind {
                CaveKind::Cheese => 0,
                CaveKind::Dripstone => 1,
                CaveKind::Spaghetti => 2,
            }] = true;
            if let Some(center) = terrain.cave_chamber_center(px, pz) {
                assert!(terrain.sample(center).density > 0.0);
            }
            for y in [-96.0, -48.0, -16.0] {
                let boundary = f64::from(x) * 192.0;
                assert!(
                    (terrain.tunnel_density([boundary - 1e-5, y, pz])
                        - terrain.tunnel_density([boundary + 1e-5, y, pz]))
                    .abs()
                        < 0.001
                );
            }
        }
    }
    assert_eq!(found, [true; 3]);
}
